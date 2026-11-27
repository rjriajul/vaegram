use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use super::ConversationUpdate;
use super::handle::Conversation;
use super::inbox::private::Wanted;
use crate::errors::{Error, Result};
use crate::filters::Filter;
use crate::types::{CallbackQuery, Message};

/// The future an `otherwise` callback returns, boxed so a wait can borrow the conversation's locals.
pub(crate) type Step<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

/// What a conversation runs for an update a wait doesn't want.
pub(crate) type Otherwise<'a> = Box<dyn FnMut(Answer) -> Step<'a> + Send + 'a>;

pub(crate) fn otherwise<'a, T, F, Fut, R>(mut f: F) -> Otherwise<'a>
where
    T: ConversationUpdate,
    F: FnMut(T) -> Fut + Send + 'a,
    Fut: Future<Output = Result<R>> + Send + 'a,
{
    Box::new(move |answer: Answer| match T::from_answer(answer) {
        Some(item) => {
            let future = f(item);
            Box::pin(async move { future.await.map(drop) }) as Step<'a>
        }
        // A wait for one kind only ever answers that kind.
        None => Box::pin(std::future::ready(Ok(()))) as Step<'a>,
    })
}

/// An update a conversation got from [`Conversation::wait_either`]: a new message or a button press.
// Short-lived: each one is matched and moved out once, so boxing the larger variant would only add an allocation.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum Answer {
    /// A new message.
    Message(Message),
    /// A button press.
    CallbackQuery(CallbackQuery),
}

#[must_use = "a wait does nothing until it is awaited"]
/// A wait for the conversation's next update of kind `T`, from [`Conversation::wait`] and the other `wait_*`
/// methods. Awaiting it returns the first update that passes its filter.
///
/// An update of kind `T` that fails the filter goes on to the client's handlers, as if the conversation weren't
/// there, unless [`otherwise`](Self::otherwise) is set: then the conversation keeps it and runs `otherwise` with it.
/// A command is an ordinary text answer: it passes `filters::text()` unless the conversation ignores it or the
/// filter excludes it (`.and(!filters::command("cancel"))`). Updates of the other kind always go on to the handlers
/// while the wait lasts. It fails with `Error::ListenerTimeout` after the conversation's timeout (the client's
/// `listener_timeout`, 5 minutes, unless set), and with `Error::ListenerStopped` when the client stops; returning
/// either with `?` ends the conversation without reporting it.
pub struct Wait<'a, T> {
    conversation: &'a mut Conversation,
    filter: Option<Filter<T>>,
    pins: Vec<i32>,
    otherwise: Option<Otherwise<'a>>,
    timeout: Option<Option<Duration>>,
}

impl<'a, T: ConversationUpdate> Wait<'a, T> {
    pub(crate) fn new(conversation: &'a mut Conversation, filter: Option<Filter<T>>) -> Self {
        Wait { conversation, filter, pins: Vec::new(), otherwise: None, timeout: None }
    }

    /// Also requires `filter` to pass: the update must pass both.
    pub fn and(mut self, filter: Filter<T>) -> Self {
        self.filter = Some(match self.filter.take() {
            Some(first) => first & filter,
            None => filter,
        });
        self
    }

    /// Runs `f` with each update of kind `T` that fails the filter, then waits on; the update goes no further.
    /// Use it to ask again: `.otherwise(|m: Message| async move { m.reply("A number, please.").await })`. An
    /// `Err` from `f` ends the wait with that error. In a parallel conversation, setting it makes the wait fail
    /// with [`ConversationError::OtherwiseInParallel`](super::ConversationError::OtherwiseInParallel) when awaited:
    /// it would take updates the other conversations wait for.
    pub fn otherwise<F, Fut, R>(mut self, f: F) -> Self
    where
        F: FnMut(T) -> Fut + Send + 'a,
        Fut: Future<Output = Result<R>> + Send + 'a,
    {
        self.otherwise = Some(otherwise(f));
        self
    }

    /// How long to wait; the conversation's timeout unless set.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(Some(timeout));
        self
    }

    /// Waits for as long as it takes.
    pub fn no_timeout(mut self) -> Self {
        self.timeout = Some(None);
        self
    }

    async fn send(self) -> Result<T> {
        let Wait { conversation, filter, pins, otherwise, timeout } = self;
        let wanted = Wanted { pins, ..T::wanted(filter.map(Arc::new)) };
        let answer = conversation.next(wanted, otherwise, timeout).await?;
        T::from_answer(answer).ok_or(Error::Unexpected("a conversation's wait got an update of another kind"))
    }
}

impl Wait<'_, CallbackQuery> {
    /// Only presses on the buttons of this message, such as the keyboard the conversation just sent; call again to
    /// accept another message too. Under the topic scopes a wait for presses needs it.
    pub fn message_id(mut self, message_id: i32) -> Self {
        self.pins.push(message_id);
        self
    }
}

impl<'a, T: ConversationUpdate> IntoFuture for Wait<'a, T> {
    type Output = Result<T>;
    type IntoFuture = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

#[must_use = "a wait does nothing until it is awaited"]
/// A wait for a new message or a button press, whichever passes its filter first, from
/// [`Conversation::wait_either`]. Updates that pass neither go on to the client's handlers; the timeout works as on
/// [`Wait`].
pub struct WaitEither<'a> {
    conversation: &'a mut Conversation,
    messages: Filter<Message>,
    presses: Filter<CallbackQuery>,
    pins: Vec<i32>,
    timeout: Option<Option<Duration>>,
}

impl<'a> WaitEither<'a> {
    pub(crate) fn new(
        conversation: &'a mut Conversation,
        messages: Filter<Message>,
        presses: Filter<CallbackQuery>,
    ) -> Self {
        WaitEither { conversation, messages, presses, pins: Vec::new(), timeout: None }
    }

    /// Only presses on the buttons of this message; call again to accept another message too. Under the topic
    /// scopes it is needed.
    pub fn message_id(mut self, message_id: i32) -> Self {
        self.pins.push(message_id);
        self
    }

    /// How long to wait; the conversation's timeout unless set.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(Some(timeout));
        self
    }

    /// Waits for as long as it takes.
    pub fn no_timeout(mut self) -> Self {
        self.timeout = Some(None);
        self
    }

    async fn send(self) -> Result<Answer> {
        let WaitEither { conversation, messages, presses, pins, timeout } = self;
        let callbacks = Some(Some(Arc::new(presses)));
        let wanted = Wanted { messages: Some(Some(Arc::new(messages))), callbacks, pins };
        conversation.next(wanted, None, timeout).await
    }
}

impl<'a> IntoFuture for WaitEither<'a> {
    type Output = Result<Answer>;
    type IntoFuture = Pin<Box<dyn Future<Output = Result<Answer>> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}
