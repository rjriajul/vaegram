use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use super::handle::Conversation;
use super::inbox::private::Wanted;
use super::wait::{Answer, Otherwise, Step, otherwise};
use crate::errors::{Error, Result};
use crate::filters::{self, Filter};
use crate::types::{Message, Photo};

type Read<V> = Arc<dyn Fn(&Message) -> Option<V> + Send + Sync>;

/// The form fields a conversation can wait for, from [`Conversation::form`]. Each returns a [`Field`] to await.
pub struct Form<'a> {
    conversation: &'a mut Conversation,
}

impl<'a> Form<'a> {
    pub(crate) fn new(conversation: &'a mut Conversation) -> Self {
        Form { conversation }
    }

    /// The text of the next text message, commands included.
    pub fn text(self) -> Field<'a, String> {
        self.read(filters::text(), |m| Some(m.text().to_owned()))
    }

    /// The next text message parsed as `N` (an integer, a float, or any [`FromStr`] type), spaces around it
    /// trimmed: `let age: u8 = conversation.form().number().await?;`. Text that doesn't parse, `300` for a `u8`
    /// included, goes to the field's `otherwise`.
    pub fn number<N: FromStr + Send + 'static>(self) -> Field<'a, N> {
        self.read(filters::text(), |m| m.text().trim().parse().ok())
    }

    /// The next text message that is one of `options`, spaces around it trimmed, compared exactly; it returns the
    /// option.
    pub fn select<S: Into<String>>(self, options: impl IntoIterator<Item = S>) -> Field<'a, String> {
        let options: Vec<String> = options.into_iter().map(Into::into).collect();
        self.read(filters::text(), move |m| {
            let text = m.text().trim();
            options.iter().find(|option| option.as_str() == text).cloned()
        })
    }

    /// The photo of the next message that has one.
    pub fn photo(self) -> Field<'a, Photo> {
        self.read(filters::photo(), |m| m.photo().cloned())
    }

    /// A field of your own: the first message `read` returns a value for.
    pub fn field<V: Send + 'static>(
        self,
        read: impl Fn(&Message) -> Option<V> + Send + Sync + 'static,
    ) -> Field<'a, V> {
        self.read(filters::all(), read)
    }

    fn read<V: 'static>(
        self,
        base: Filter<Message>,
        read: impl Fn(&Message) -> Option<V> + Send + Sync + 'static,
    ) -> Field<'a, V> {
        let read: Read<V> = Arc::new(read);
        let check = Arc::clone(&read);
        let filter = base & filters::create(move |m: &Message| check(m).is_some());
        Field { conversation: self.conversation, filter, read, otherwise: None, timeout: None }
    }
}

#[must_use = "a form field does nothing until it is awaited"]
/// One form field to wait for, from [`Form`]. Awaiting it returns the value of the first new message it can read.
///
/// Unlike a [`Wait`](super::Wait), a field keeps the messages it can't read: [`otherwise`](Self::otherwise) runs
/// with each, or without one they are dropped, so a typo never reaches an unrelated handler. In a parallel
/// conversation they go on to the handlers instead. A command the conversation doesn't ignore is a message like any
/// other: `text()` takes `/cancel`. Button presses always go on to the handlers. Timeouts work as on a wait.
pub struct Field<'a, V> {
    conversation: &'a mut Conversation,
    filter: Filter<Message>,
    read: Read<V>,
    otherwise: Option<Otherwise<'a>>,
    timeout: Option<Option<Duration>>,
}

impl<'a, V: Send + 'static> Field<'a, V> {
    /// Runs `f` with each message the field can't read, then waits on: use it to ask again. In a parallel
    /// conversation, setting it makes the field fail with
    /// [`ConversationError::OtherwiseInParallel`](super::ConversationError::OtherwiseInParallel) when awaited.
    pub fn otherwise<F, Fut, R>(mut self, f: F) -> Self
    where
        F: FnMut(Message) -> Fut + Send + 'a,
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

    async fn send(self) -> Result<V> {
        let Field { conversation, filter, read, otherwise, timeout } = self;
        let drop_rest = || -> Otherwise<'a> { Box::new(|_: Answer| Box::pin(std::future::ready(Ok(()))) as Step<'a>) };
        let otherwise = otherwise.or_else(|| (!conversation.parallel()).then(drop_rest));
        let wanted = Wanted { messages: Some(Some(Arc::new(filter))), callbacks: None, pins: Vec::new() };
        let Answer::Message(message) = conversation.next(wanted, otherwise, timeout).await? else {
            return Err(Error::Unexpected("a form field got a button press"));
        };
        read(&message).ok_or(Error::Unexpected("a form field could not read the message it took"))
    }
}

impl<'a, V: Send + 'static> IntoFuture for Field<'a, V> {
    type Output = Result<V>;
    type IntoFuture = Pin<Box<dyn Future<Output = Result<V>> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}
