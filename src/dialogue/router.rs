use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use super::key::{sender, topic};
use super::locks::KeyLocks;
use super::{Dialogue, DialogueKey, DialogueStorage, Scope};
use crate::errors::Result;
use crate::filters::{Filter, StateFilter};
use crate::handlers::{Next, Router, UpdateContext};
use crate::types::{CallbackOrigin, CallbackQuery, Message};

type StepFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;
type StepHandler<S, T> = Arc<dyn Fn(T, Dialogue<S>) -> StepFuture + Send + Sync>;

struct Step<S, T> {
    state: StateFilter<S>,
    filter: Filter<T>,
    handler: StepHandler<S, T>,
}

impl<S, T> Clone for Step<S, T> {
    fn clone(&self) -> Self {
        Step { state: self.state.clone(), filter: self.filter.clone(), handler: Arc::clone(&self.handler) }
    }
}

fn step_handler<S, T, F, Fut, R>(f: F) -> StepHandler<S, T>
where
    F: Fn(T, Dialogue<S>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<R>> + Send + 'static,
{
    Arc::new(move |item, dialogue| {
        let future = f(item, dialogue);
        Box::pin(async move { future.await.map(drop) }) as StepFuture
    })
}

/// A multi-step conversation: its steps, the [`DialogueStorage`] that keeps each conversation's state of type `S`,
/// and whose conversation an update belongs to ([`Scope`]). Mount it with [`into_router`](Self::into_router).
///
/// Each new message and callback query that reaches it goes through its steps in the order they were added. The
/// first step whose update filter passes and whose [`StateFilter`] accepts the conversation's state runs, with a
/// [`Dialogue`] to read and change that state, and takes the turn of the group the dialogue is included in: routes
/// in later groups still get the update, so a catch-all there (an "unknown command" reply, say) would answer it too,
/// unless the include stops them: `client.include_router(flow.into_router()).stop()`. When no step takes the
/// update, it goes on to the routes after this one, as if the dialogue weren't there.
///
/// The conversation stays locked from loading its state until the step returns, so two quick messages from one user
/// run one after the other, even with handlers running concurrently. An update waiting for its conversation's turn
/// holds none of the `max_concurrent_handlers` slots, so other conversations go on meanwhile.
///
/// A step may still wait for a reply with [`Client::ask`](crate::Client::ask) or `listen`: replies reach those
/// before any router. But the conversation stays locked while it waits, so its next updates (a `/cancel` too)
/// wait with it; a step per question avoids that.
///
/// ```no_run
/// use vaegram::dialogue::{Dialogue, DialogueRouter, MemoryStorage};
/// use vaegram::prelude::*;
///
/// #[derive(Clone)]
/// enum Signup {
///     Name,
///     Age { name: String },
/// }
///
/// # fn f(client: Client) {
/// let mut signup = DialogueRouter::new("signup", MemoryStorage::new());
/// signup.on_message(filters::no_state(), filters::command("signup"), |msg, dlg: Dialogue<Signup>| async move {
///     dlg.set(Signup::Name).await?;
///     msg.reply("Your name?").await
/// });
/// let asked_name = filters::state(|s: &Signup| matches!(s, Signup::Name));
/// signup.on_message(asked_name, filters::text(), |msg, dlg: Dialogue<Signup>| async move {
///     dlg.set(Signup::Age { name: msg.text().to_owned() }).await?;
///     msg.reply("Your age?").await
/// });
/// client.include_router(signup.into_router());
/// # }
/// ```
pub struct DialogueRouter<S> {
    namespace: Arc<str>,
    storage: Arc<dyn DialogueStorage<S>>,
    scope: Scope,
    timeout: Option<Duration>,
    locks: Arc<KeyLocks>,
    messages: Vec<Step<S, Message>>,
    callback_queries: Vec<Step<S, CallbackQuery>>,
}

impl<S> Clone for DialogueRouter<S> {
    fn clone(&self) -> Self {
        DialogueRouter {
            namespace: Arc::clone(&self.namespace),
            storage: Arc::clone(&self.storage),
            scope: self.scope,
            timeout: self.timeout,
            locks: Arc::clone(&self.locks),
            messages: self.messages.clone(),
            callback_queries: self.callback_queries.clone(),
        }
    }
}

impl<S: Clone + Send + Sync + 'static> DialogueRouter<S> {
    /// A dialogue with no steps yet, keeping its states in `storage` under `namespace`, keyed by user in each chat
    /// and never timing out. Dialogues that share a storage need different namespaces.
    pub fn new(namespace: impl Into<Arc<str>>, storage: impl DialogueStorage<S>) -> Self {
        DialogueRouter {
            namespace: namespace.into(),
            storage: Arc::new(storage),
            scope: Scope::default(),
            timeout: None,
            locks: Arc::default(),
            messages: Vec::new(),
            callback_queries: Vec::new(),
        }
    }

    /// Keys conversations by `scope` instead of [`Scope::UserInChat`].
    #[must_use]
    pub fn scope(mut self, scope: Scope) -> Self {
        self.scope = scope;
        self
    }

    /// Ends a conversation `timeout` after its state last changed: the next update finds no state, as after
    /// [`Dialogue::clear`]. Without it, a state stays until it's cleared.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Adds a step for new messages that pass `filter` while the conversation's state passes `state`.
    pub fn on_message<F, Fut, R>(&mut self, state: StateFilter<S>, filter: Filter<Message>, f: F) -> &mut Self
    where
        F: Fn(Message, Dialogue<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<R>> + Send + 'static,
    {
        self.messages.push(Step { state, filter, handler: step_handler(f) });
        self
    }

    /// Adds a step for callback queries that pass `filter` while the conversation's state passes `state`.
    ///
    /// Presses of a button on an inline message have no chat, so only [`Scope::User`] keys them; under the other
    /// scopes no step gets them. Under the topic scopes the pressed message is read for its topic, from the message
    /// cache or else from Telegram.
    pub fn on_callback_query<F, Fut, R>(
        &mut self,
        state: StateFilter<S>,
        filter: Filter<CallbackQuery>,
        f: F,
    ) -> &mut Self
    where
        F: Fn(CallbackQuery, Dialogue<S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<R>> + Send + 'static,
    {
        self.callback_queries.push(Step { state, filter, handler: step_handler(f) });
        self
    }

    /// The key of `user_id`'s conversation in `chat_id` in this dialogue, outside topics. Pass `0` for what the
    /// dialogue's [`Scope`] doesn't key by.
    pub fn key(&self, chat_id: i64, user_id: i64) -> DialogueKey {
        DialogueKey::new(Arc::clone(&self.namespace), chat_id, user_id)
    }

    /// The key of the conversation `message` belongs to, or `None` when the scope needs a sender it hasn't got.
    pub fn key_for(&self, message: &Message) -> Option<DialogueKey> {
        let business = message.business_connection_id.as_deref();
        self.scope.key(&self.namespace, Some(message.chat.id), sender(message), topic(message), business)
    }

    /// The conversation under `key`, to read or change outside a step, say to reset a user from an admin command.
    /// It takes no lock: a step of the same conversation running meanwhile may overwrite what it saves.
    pub async fn dialogue(&self, key: DialogueKey) -> Result<Dialogue<S>> {
        Dialogue::open(Arc::clone(&self.storage), key, self.timeout).await
    }

    /// Runs `message` through the steps, as the router from [`into_router`](Self::into_router) does; `true` if a
    /// step took it. For code that routes updates itself.
    pub async fn handle_message(&self, message: &Message) -> Result<bool> {
        match self.pick_message(message) {
            Some((message, steps, key)) => self.run(&self.messages, steps, message, key).await,
            None => Ok(false),
        }
    }

    /// Runs `query` through the steps, as the router from [`into_router`](Self::into_router) does; `true` if a
    /// step took it.
    pub async fn handle_callback_query(&self, query: &CallbackQuery) -> Result<bool> {
        match self.pick_callback_query(query) {
            Some((query, steps)) => match self.callback_query_key(&query).await {
                Some(key) => self.run(&self.callback_queries, steps, query, key).await,
                None => Ok(false),
            },
            None => Ok(false),
        }
    }

    /// A [`Router`] that runs the steps: include it in a client or another router. Updates no step takes go on to
    /// the routes after it.
    pub fn into_router(self) -> Router {
        let flow = Arc::new(self);
        let mut router = Router::new();
        if !flow.messages.is_empty() {
            let flow = Arc::clone(&flow);
            router
                .message()
                .outer_middleware(move |ctx: UpdateContext, next: Next| route_message(Arc::clone(&flow), ctx, next));
        }
        if !flow.callback_queries.is_empty() {
            let flow = Arc::clone(&flow);
            router.callback_query().outer_middleware(move |ctx: UpdateContext, next: Next| {
                route_callback_query(Arc::clone(&flow), ctx, next)
            });
        }
        router
    }

    /// The indexes of the message steps whose filter `message` passes, with a copy of it and its key; `None` when
    /// no step can take it, without loading any state.
    fn pick_message(&self, message: &Message) -> Option<(Message, Vec<usize>, DialogueKey)> {
        let steps = passing(&self.messages, |filter| filter.check(message))?;
        let key = self.key_for(message)?;
        Some((message.clone(), steps, key))
    }

    fn pick_callback_query(&self, query: &CallbackQuery) -> Option<(CallbackQuery, Vec<usize>)> {
        let steps = passing(&self.callback_queries, |filter| filter.check(query))?;
        Some((query.clone(), steps))
    }

    /// The key of the conversation `query` belongs to. Under the topic scopes the topic comes from the pressed
    /// message: an ephemeral one carries it, a regular one is read from the cache or Telegram. When that message
    /// can't be read (deleted, or the fetch failed), the press has no key and goes on to later routes.
    async fn callback_query_key(&self, query: &CallbackQuery) -> Option<DialogueKey> {
        let thread = if !self.scope.by_topic() {
            None
        } else if let CallbackOrigin::Ephemeral(message) = &query.origin {
            // Outside forums `top_msg_id` can name the pressed message rather than a topic.
            if message.chat.is_forum { message.message_thread_id } else { None }
        } else {
            let message = query.message().await.ok().flatten()?;
            topic(&message)
        };
        let chat = query.chat().map(|chat| chat.id);
        let business = query.business_connection_id();
        self.scope.key(&self.namespace, chat, Some(query.from_user.id), thread, business)
    }

    /// Locks `key`'s conversation, loads its state and runs the first of `candidates` whose state filter accepts
    /// it. `false` when none does.
    async fn run<T: Send + 'static>(
        &self,
        steps: &[Step<S, T>],
        candidates: Vec<usize>,
        item: T,
        key: DialogueKey,
    ) -> Result<bool> {
        let lock = self.locks.get(&key);
        // Waiting for the conversation's turn, the task hands its handler slot back, so a burst from one
        // conversation can't hold every slot while other chats wait.
        let _turn = crate::dispatcher::without_slot(lock.lock()).await;
        let dialogue = Dialogue::open(Arc::clone(&self.storage), key, self.timeout).await?;
        let state = dialogue.state();
        let step = candidates.into_iter().map(|i| &steps[i]).find(|step| step.state.check(state.as_ref()));
        let Some(step) = step else { return Ok(false) };
        (step.handler)(item, dialogue).await?;
        Ok(true)
    }
}

/// The indexes of `steps` whose filter passes `test`; `None` when none does.
fn passing<S, T>(steps: &[Step<S, T>], test: impl Fn(&Filter<T>) -> bool) -> Option<Vec<usize>> {
    let found: Vec<usize> = steps.iter().enumerate().filter(|(_, step)| test(&step.filter)).map(|(i, _)| i).collect();
    (!found.is_empty()).then_some(found)
}

async fn route_message<S: Clone + Send + Sync + 'static>(
    flow: Arc<DialogueRouter<S>>,
    ctx: UpdateContext,
    next: Next,
) -> Result<()> {
    let picked = ctx.message().and_then(|message| flow.pick_message(message));
    if let Some((message, steps, key)) = picked {
        if flow.run(&flow.messages, steps, message, key).await? {
            return Ok(());
        }
    }
    next.run(ctx).await
}

async fn route_callback_query<S: Clone + Send + Sync + 'static>(
    flow: Arc<DialogueRouter<S>>,
    ctx: UpdateContext,
    next: Next,
) -> Result<()> {
    let picked = ctx.callback_query().and_then(|query| flow.pick_callback_query(query));
    if let Some((query, steps)) = picked {
        if let Some(key) = flow.callback_query_key(&query).await {
            if flow.run(&flow.callback_queries, steps, query, key).await? {
                return Ok(());
            }
        }
    }
    next.run(ctx).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Client;
    use crate::dialogue::MemoryStorage;
    use crate::dispatcher::Entities;
    use crate::dispatcher::listeners::tests::query;
    use crate::errors::{Error, RpcError};
    use crate::filters;
    use crate::raw::{Serializable, base, types};
    use crate::utils::get_peer;

    /// The answer to `messages.getMessages`: `messages`, and no users or chats.
    fn found(messages: Vec<base::Message>) -> Vec<u8> {
        let found = types::messages::Messages { messages, topics: Vec::new(), chats: Vec::new(), users: Vec::new() };
        base::messages::Messages::Messages(Box::new(found)).to_bytes()
    }

    /// Message `id` of `chat`, in the forum topic `topic`.
    fn in_topic(id: i32, chat: i64, topic: i32) -> base::Message {
        let header =
            types::MessageReplyHeader { forum_topic: true, reply_to_msg_id: Some(topic), ..Default::default() };
        let reply_to = Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(header)));
        base::Message::Message(Box::new(types::Message { reply_to, ..Message::bare(id, get_peer(chat), 0) }))
    }

    /// A dialogue whose one step takes every press and moves its conversation to `2`.
    fn pressing(scope: Scope) -> DialogueRouter<u8> {
        let mut flow = DialogueRouter::new("press", MemoryStorage::new()).scope(scope);
        let always = filters::any_state() | filters::no_state();
        flow.on_callback_query(always, filters::all(), |_query, dlg: Dialogue<u8>| async move { dlg.set(2).await });
        flow
    }

    async fn state(flow: &DialogueRouter<u8>, key: DialogueKey) -> Option<u8> {
        flow.dialogue(key).await.unwrap().state()
    }

    #[tokio::test]
    async fn a_topic_press_keys_into_the_pressed_messages_topic() {
        let (client, _sent) = Client::scripted_raw(vec![Ok(found(vec![in_topic(5, -5, 10)]))]);
        let flow = pressing(Scope::UserInTopic);
        let other = flow.key(-5, 7).thread(Some(20));
        flow.dialogue(other.clone()).await.unwrap().set(1).await.unwrap();

        assert!(flow.handle_callback_query(&query(&client, 7, -5, 5)).await.unwrap());
        assert_eq!(state(&flow, flow.key(-5, 7).thread(Some(10))).await, Some(2));
        assert_eq!(state(&flow, other).await, Some(1), "another topic's conversation moved");
        assert_eq!(state(&flow, flow.key(-5, 7)).await, None, "the press was keyed outside its topic");
    }

    #[tokio::test]
    async fn a_press_whose_message_cannot_be_read_is_left_to_later_routes() {
        let flow = pressing(Scope::UserInTopic);
        // The pressed message is gone.
        let (client, _sent) = Client::scripted_raw(vec![Ok(found(Vec::new()))]);
        assert!(!flow.handle_callback_query(&query(&client, 7, -5, 5)).await.unwrap());
        // Reading it fails.
        let invalid = Error::Rpc(RpcError::new(400, "MESSAGE_ID_INVALID", None));
        let (client, _sent) = Client::scripted_raw(vec![Err(invalid)]);
        assert!(!flow.handle_callback_query(&query(&client, 7, -5, 5)).await.unwrap());
        assert_eq!(state(&flow, flow.key(-5, 7)).await, None);
    }

    #[tokio::test]
    async fn a_business_press_does_not_take_the_bots_own_conversation() {
        let flow = pressing(Scope::UserInChat);
        let own = flow.key(7, 7);
        flow.dialogue(own.clone()).await.unwrap().set(1).await.unwrap();
        let update = types::UpdateBusinessBotCallbackQuery {
            query_id: 1,
            user_id: 7,
            connection_id: "conn".into(),
            message: base::Message::Message(Box::new(Message::bare(5, get_peer(7), 0))),
            reply_to_message: None,
            chat_instance: 3,
            data: Some(b"yes".to_vec()),
        };
        let press = CallbackQuery::parse_business(&Client::offline(), update, &Entities::default());

        assert!(flow.handle_callback_query(&press).await.unwrap());
        assert_eq!(state(&flow, own).await, Some(1), "the press moved the private chat's conversation");
        assert_eq!(state(&flow, flow.key(7, 7).business_connection(Some("conn"))).await, Some(2));
    }
}
