//! Conversations: a multi-step flow written as one async function that waits for each answer in place.
//!
//! [`Client::conversation`](crate::Client::conversation) registers the function under a name; a handler starts it
//! for a user with [`Client::enter_conversation`](crate::Client::enter_conversation). The function gets a
//! [`Conversation`] and the update that started it, and awaits the next update it needs with
//! [`wait`](Conversation::wait), [`wait_for`](Conversation::wait_for),
//! [`wait_for_callback_query`](Conversation::wait_for_callback_query) and the other `wait_*` methods, or reads a
//! typed value with [`form`](Conversation::form). Code between the waits runs once, as written: no replay, so
//! database calls, random numbers and the time need no wrapping.
//!
//! ```no_run
//! use std::time::Duration;
//!
//! use vaegram::prelude::*;
//!
//! # fn f(client: &Client) {
//! client
//!     .conversation("signup", |mut conv: Conversation, msg: Message| async move {
//!         msg.reply("What's your name?").await?;
//!         let name = conv.wait_for(filters::text()).await?.text().to_owned();
//!         msg.reply("How old are you?").await?;
//!         let age: u8 = conv
//!             .form()
//!             .number()
//!             .otherwise(|m: Message| async move { m.reply("A number, please.").await })
//!             .await?;
//!         msg.reply(format!("Welcome, {name} ({age})!")).await?;
//!         Ok(())
//!     })
//!     .timeout(Duration::from_secs(600))
//!     .ignore_commands();
//!
//! client.on_message(filters::command("start"), |msg| async move {
//!     msg.client().enter_conversation("signup", msg.clone())
//! });
//! client.on_message(filters::command("cancel"), |msg| async move {
//!     msg.client().exit_conversation("signup", &msg);
//!     msg.reply("Cancelled.").await
//! });
//! # }
//! ```
//!
//! # How it runs
//!
//! A started conversation runs on a task of its own. While it waits it holds no handler slot and no turn of its
//! chat, and it costs one `max_listeners` slot (spread over one listener per update kind) and a parked task: each
//! update that reaches it resumes the function where it waited. Updates its wait doesn't want go on to the
//! handlers, unless the wait has `otherwise`; form fields keep the messages they can't read. A command is an
//! ordinary text answer unless the conversation ignores it ([`ignore`](ConversationRef::ignore),
//! [`ignore_commands`](ConversationRef::ignore_commands)) or the wait's filter excludes it. One conversation runs
//! per user in each chat by default ([`Scope`]); a second
//! [`enter_conversation`](crate::Client::enter_conversation) there fails with [`ConversationError::Busy`] until the
//! first ends, unless both are [`parallel`](ConversationRef::parallel). Under the topic scopes, start it from a
//! message, and pin each wait for presses to its keyboard with [`message_id`](Wait::message_id): a press doesn't
//! say its topic. Updates through a business connection never reach a conversation.
//!
//! # Conversations, `ask` and dialogues
//!
//! [`Client::ask`](crate::Client::ask) and `listen` wait for one reply inside a handler, which keeps its handler
//! slot meanwhile; a conversation is a whole flow on its own task. Both wait in memory, so a restart ends them. A
//! [`DialogueRouter`](crate::dialogue::DialogueRouter) keeps a flow's state in a storage instead, one handler per
//! step, and carries on after a restart with a persistent storage: use it for flows that must survive one.

mod form;
mod handle;
mod inbox;
mod registry;
mod wait;

use std::sync::Arc;

pub use form::{Field, Form};
pub use handle::Conversation;
pub use registry::{ConversationError, ConversationRef};
pub use wait::{Answer, Wait, WaitEither};

pub(crate) use registry::Registry;

use self::inbox::private::{Item, Topic, Uid, Wanted};
pub use crate::dialogue::Scope;
use crate::dispatcher::listeners::Listened;
use crate::errors::Error;
use crate::filters::Filter;
use crate::handlers::{HandlerError, Listenable, Routed};
use crate::types::{CallbackQuery, Message};

/// An update kind a conversation starts with and waits for: [`Message`] or [`CallbackQuery`].
pub trait ConversationUpdate: Listenable + Clone + Send + Sync + 'static + Item {}

impl ConversationUpdate for Message {}

impl ConversationUpdate for CallbackQuery {}

fn failure<T: Routed + Listened>(item: &T, error: Error) -> HandlerError {
    HandlerError {
        kind: T::KIND,
        chat_id: Routed::chat_id(item),
        message_id: Routed::message_id(item),
        user_id: Routed::user_id(item),
        query_id: Routed::query_id(item),
        error,
    }
}

impl Item for Message {
    fn parties(&self) -> (Option<i64>, Option<i64>) {
        // As the listeners see it: the sender, or the chat it was sent on behalf of.
        let user = self.from_user.as_ref().map(|u| u.id).or(self.sender_chat.as_ref().map(|c| c.id));
        (Some(self.chat.id), user)
    }

    fn conversation_topic(&self) -> Topic {
        Topic::Known(if self.is_topic_message { self.message_thread_id } else { None })
    }

    fn via_business(&self) -> bool {
        self.business_connection_id.is_some()
    }

    fn uid(&self) -> Uid {
        Uid::Message(self.chat.id, self.id)
    }

    fn into_answer(self) -> Answer {
        Answer::Message(self)
    }

    fn from_answer(answer: Answer) -> Option<Self> {
        match answer {
            Answer::Message(message) => Some(message),
            Answer::CallbackQuery(_) => None,
        }
    }

    fn failure(&self, error: Error) -> HandlerError {
        failure(self, error)
    }

    fn wanted(filter: Option<Arc<Filter<Self>>>) -> Wanted {
        Wanted { messages: Some(filter), callbacks: None, pins: Vec::new() }
    }
}

impl Item for CallbackQuery {
    fn parties(&self) -> (Option<i64>, Option<i64>) {
        (self.chat().map(|c| c.id), Some(self.from_user.id))
    }

    fn conversation_topic(&self) -> Topic {
        Topic::Unknown
    }

    fn via_business(&self) -> bool {
        self.business_connection_id().is_some()
    }

    fn uid(&self) -> Uid {
        Uid::CallbackQuery(self.id)
    }

    fn into_answer(self) -> Answer {
        Answer::CallbackQuery(self)
    }

    fn from_answer(answer: Answer) -> Option<Self> {
        match answer {
            Answer::CallbackQuery(query) => Some(query),
            Answer::Message(_) => None,
        }
    }

    fn failure(&self, error: Error) -> HandlerError {
        failure(self, error)
    }

    fn wanted(filter: Option<Arc<Filter<Self>>>) -> Wanted {
        Wanted { messages: None, callbacks: Some(filter), pins: Vec::new() }
    }
}
