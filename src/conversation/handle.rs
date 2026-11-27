use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;

use super::form::Form;
use super::inbox::private::{Item, Wanted};
use super::inbox::{self, Feeds, Inbox, Shared};
use super::registry::{Key, Options};
use super::wait::{Answer, Otherwise, Wait, WaitEither};
use super::{ConversationError, ConversationUpdate};
use crate::Client;
use crate::dispatcher::listeners::Listened;
use crate::errors::{Error, Result};
use crate::filters::{self, CommandNames, Filter};
use crate::types::{CallbackQuery, Message};

/// One running conversation, handed to the function [`Client::conversation`] registered. Its `wait` methods pause
/// the function until the next update that fits, then return that update; meanwhile the conversation holds no
/// handler slot and no turn of its chat, so other chats, and the same chat's other users, go on.
///
/// It sees the new messages and button presses of its chat and user (its [`Scope`](crate::dialogue::Scope)), and
/// only those: never edits, and never updates that come through a business connection. Under the topic scopes it
/// takes only the presses on messages its wait names with [`message_id`](Wait::message_id), since a press doesn't
/// say its topic. Updates that arrive while its own code runs between two waits are kept, up to 64 in all, for the
/// next wait, so an answer that comes before the question's `send_message` returns still counts; the next wait
/// passes on to the handlers the ones it doesn't want. A parallel conversation keeps none, so it never holds up the
/// updates another one waits for.
///
/// A command is an ordinary text answer: `/cancel` sent while the conversation waits for a name becomes the name,
/// unless the conversation [`ignore`](super::ConversationRef::ignore)s it (or
/// [`ignore_commands`](super::ConversationRef::ignore_commands)) or the wait's filter excludes it, as with
/// `filters::text() & !filters::command("cancel")`.
///
/// It costs one `max_listeners` slot, spread over one listener per update kind. A listener started after the
/// conversation, by [`Client::ask`] or `listen`, never gets what the conversation takes: wait with the
/// conversation's own methods inside it. A listener started before it takes the answer first. `stop_listening`
/// leaves conversations alone; [`Client::exit_conversation`] ends one.
///
/// The conversation ends when the function returns, fails or panics, when a wait times out, when
/// [`Client::exit_conversation`] ends it, or when the client stops. Updates it took and never read then go on to
/// the handlers, never to another conversation. It lives in memory only: a restart ends it. For a flow that must
/// outlive restarts, keep its state in a [`DialogueRouter`](crate::dialogue::DialogueRouter) storage instead.
pub struct Conversation {
    client: Client,
    name: Arc<str>,
    key: Key,
    feeds: Feeds,
    shared: Arc<Shared>,
    inbox: Inbox,
    timeout: Option<Duration>,
    parallel: bool,
}

impl Conversation {
    /// Starts the listeners of a conversation under `key`, so nothing in its chat is missed from here on.
    pub(crate) fn open(client: &Client, name: Arc<str>, key: Key, options: &Options) -> Result<Conversation> {
        let shared = Arc::new(Shared::new(options, &key, client.0.conversations.drained()));
        let (feeds, inbox) = inbox::open(client, &key, &shared)?;
        let timeout = options.timeout.unwrap_or(client.0.listener_timeout);
        let parallel = options.parallel;
        Ok(Conversation { client: client.clone(), name, key, feeds, shared, inbox, timeout, parallel })
    }

    pub(crate) fn feeds(&self) -> Feeds {
        self.feeds
    }

    pub(crate) fn parallel(&self) -> bool {
        self.parallel
    }

    /// The client the conversation runs on.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// The name the conversation was registered under.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The chat the conversation belongs to; `None` under [`Scope::User`](crate::dialogue::Scope::User), which
    /// follows its user across chats.
    pub fn chat_id(&self) -> Option<i64> {
        self.key.chat
    }

    /// The user the conversation belongs to; `None` under the scopes shared by a whole chat or topic.
    pub fn user_id(&self) -> Option<i64> {
        self.key.user
    }

    /// Waits for the next new message.
    pub fn wait(&mut self) -> Wait<'_, Message> {
        Wait::new(self, None)
    }

    /// Waits for the next new message that passes `filter`, such as `filters::text()`. A command passes
    /// `filters::text()`: add `& !filters::command(..)` to leave it to the handlers.
    pub fn wait_for(&mut self, filter: Filter<Message>) -> Wait<'_, Message> {
        Wait::new(self, Some(filter))
    }

    /// Waits for the next new message from the user `user_id`: for a conversation shared by a whole chat that
    /// needs one member's answer.
    pub fn wait_from(&mut self, user_id: i64) -> Wait<'_, Message> {
        self.wait_for(filters::user(user_id))
    }

    /// Waits for the next of these commands, as `filters::command` matches them.
    pub fn wait_for_command(&mut self, names: impl CommandNames) -> Wait<'_, Message> {
        self.wait_for(filters::command(names))
    }

    /// Waits for the next button press that passes `filter`, such as `filters::data("yes")`. Pin it to one keyboard
    /// with [`message_id`](Wait::message_id), which the topic scopes require. Answer the press yourself, as in any
    /// handler.
    pub fn wait_for_callback_query(&mut self, filter: Filter<CallbackQuery>) -> Wait<'_, CallbackQuery> {
        Wait::new(self, Some(filter))
    }

    /// Waits for a new message that passes `messages` or a button press that passes `presses`, whichever came
    /// first: for a question with a keyboard that also takes a typed answer.
    pub fn wait_either(&mut self, messages: Filter<Message>, presses: Filter<CallbackQuery>) -> WaitEither<'_> {
        WaitEither::new(self, messages, presses)
    }

    /// Waits for one form field: a text, a number, one of several options, a photo, or a value of your own. A
    /// field keeps the messages it can't read and runs its `otherwise` with them, or drops them without one.
    pub fn form(&mut self) -> Form<'_> {
        Form::new(self)
    }

    /// Passes `update`, which a wait returned, on to the client's handlers, as if the conversation had never taken
    /// it. The conversation's next waits don't take it again.
    pub fn skip<T: ConversationUpdate>(&self, update: T) {
        self.shared.skip(Item::uid(&update));
        T::book(&self.client.0.handlers.listeners).hand_back(update);
    }

    /// Ends the conversation without reporting an error: `return conversation.halt();` in the function, or
    /// `conversation.halt()?` in a helper it calls. It returns [`ConversationError::Halted`], which the
    /// conversation's runner never reports.
    pub fn halt(&self) -> Result<()> {
        Err(ConversationError::Halted.into())
    }

    /// Waits for the first update `wanted` takes. Every other update the listeners took goes to `otherwise`
    /// when it is of a wanted kind and `otherwise` is set, else on to the handlers.
    pub(crate) async fn next(
        &mut self,
        wanted: Wanted,
        mut otherwise: Option<Otherwise<'_>>,
        timeout: Option<Option<Duration>>,
    ) -> Result<Answer> {
        if otherwise.is_some() && self.parallel {
            return Err(ConversationError::OtherwiseInParallel.into());
        }
        if self.shared.by_topic() && wanted.callbacks.is_some() && wanted.pins.is_empty() {
            return Err(ConversationError::UnpinnedPress.into());
        }
        let deadline = timeout.unwrap_or(self.timeout).and_then(|timeout| Instant::now().checked_add(timeout));
        let shared = Arc::clone(&self.shared);
        let _armed = shared.arm(&wanted, otherwise.is_some());
        loop {
            let next = self.inbox.0.recv();
            let answer = match deadline {
                Some(deadline) => tokio::time::timeout_at(deadline, next).await.map_err(|_| Error::ListenerTimeout)?,
                None => next.await,
            };
            let answer = answer.ok_or(Error::ListenerStopped)?;
            match (passes(&wanted, &answer), otherwise.as_mut()) {
                (Some(true), _) => return Ok(prepare(answer).await),
                (Some(false), Some(otherwise)) => otherwise(answer).await?,
                _ => self.skip_answer(answer),
            }
        }
    }

    fn skip_answer(&self, answer: Answer) {
        match answer {
            Answer::Message(message) => self.skip(message),
            Answer::CallbackQuery(query) => self.skip(query),
        }
    }
}

/// Whether `answer` passes its kind's filter, and a press the wait's pins; `None` when the wait doesn't want its kind.
fn passes(wanted: &Wanted, answer: &Answer) -> Option<bool> {
    match answer {
        Answer::Message(m) => wanted.messages.as_ref().map(|f| f.as_ref().is_none_or(|f| f.check(m))),
        Answer::CallbackQuery(q) => {
            let pinned = wanted.pins.is_empty() || q.message_id().is_some_and(|id| wanted.pins.contains(&id));
            wanted.callbacks.as_ref().map(|f| pinned && f.as_ref().is_none_or(|f| f.check(q)))
        }
    }
}

/// Fills in what the client's fetch options ask for, as for a handler.
async fn prepare(answer: Answer) -> Answer {
    match answer {
        Answer::Message(message) => Answer::Message(message.prepare().await),
        Answer::CallbackQuery(query) => Answer::CallbackQuery(query.prepare().await),
    }
}

impl Drop for Conversation {
    fn drop(&mut self) {
        self.feeds.remove(&self.client);
        // Taken but never read: the handlers get them, oldest first. Marked first, so no conversation takes them,
        // not even one entered meanwhile under the same key.
        let listeners = &self.client.0.handlers.listeners;
        self.inbox.0.close();
        while let Ok(answer) = self.inbox.0.try_recv() {
            match answer {
                Answer::Message(message) => {
                    self.shared.drain(Item::uid(&message));
                    listeners.messages.hand_back(message);
                }
                Answer::CallbackQuery(query) => {
                    self.shared.drain(Item::uid(&query));
                    listeners.callbacks.hand_back(query);
                }
            }
        }
    }
}
