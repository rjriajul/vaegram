use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};

use super::{Next, UpdateContext};
use crate::enums::ChatAction;
use crate::errors::Result;

type MiddlewareFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;

/// A middleware that shows `action`, such as [`ChatAction::Typing`], in the update's chat while the handlers after it
/// run, sending it again every 4 seconds: in the message's forum topic, and through its business connection, when it
/// has one. Add it with `inner_middleware`, so it shows only for updates a handler takes. An update without a chat,
/// or a chat where the action can't be sent, goes on without it.
///
/// ```no_run
/// use vaegram::enums::ChatAction;
/// use vaegram::handlers::{Router, chat_action};
///
/// let mut router = Router::new();
/// router.inner_middleware(chat_action(ChatAction::Typing));
/// ```
pub fn chat_action(
    action: ChatAction,
) -> impl Fn(UpdateContext, Next) -> MiddlewareFuture + Clone + Send + Sync + 'static {
    move |ctx: UpdateContext, next: Next| -> MiddlewareFuture {
        let action = action.clone();
        Box::pin(async move {
            let client = ctx.client().clone();
            let (topic, connection) = match ctx.message() {
                Some(m) => (m.message_thread_id.filter(|_| m.is_topic_message), m.business_connection_id.clone()),
                None => (None, None),
            };
            let shown = match ctx.chat_id() {
                Some(chat_id) => {
                    let mut request = client.send_chat_action(chat_id, action);
                    if let Some(topic) = topic {
                        request = request.message_thread_id(topic);
                    }
                    if let Some(connection) = connection {
                        request = request.business_connection_id(connection);
                    }
                    request.repeat().await.ok()
                }
                None => None,
            };
            let result = next.run(ctx).await;
            // Dropping the handle stops the repeats.
            drop(shown);
            result
        })
    }
}

/// The answer [`answer_callback_queries`] sends for a button press, which its handler sets: a handler takes it as
/// a `Dep<CallbackAnswer>`. Clones share it.
#[derive(Clone, Debug, Default)]
pub struct CallbackAnswer(Arc<Mutex<Answer>>);

#[derive(Debug, Default)]
struct Answer {
    text: Option<String>,
    alert: bool,
    url: Option<String>,
    off: bool,
}

impl CallbackAnswer {
    fn with(&self, f: impl FnOnce(&mut Answer)) -> &Self {
        f(&mut self.0.lock().unwrap_or_else(PoisonError::into_inner));
        self
    }

    /// Shows `text` to the user, as a toast unless [`alert`](Self::alert) is set.
    pub fn text(&self, text: impl Into<String>) -> &Self {
        let text = text.into();
        self.with(|answer| answer.text = Some(text))
    }

    /// Shows the text as an alert the user closes.
    pub fn alert(&self) -> &Self {
        self.with(|answer| answer.alert = true)
    }

    /// Makes the client open `url`.
    pub fn url(&self, url: impl Into<String>) -> &Self {
        let url = url.into();
        self.with(|answer| answer.url = Some(url))
    }

    /// Sends no answer: the handler answers the query itself.
    pub fn disable(&self) {
        self.with(|answer| answer.off = true);
    }
}

/// A middleware that answers each button press once the handlers after it return, an `Err` included, so the
/// user's loading indicator always stops. The handler sets the answer through the [`CallbackAnswer`] it can take as
/// a `Dep<CallbackAnswer>`, or calls [`CallbackAnswer::disable`] when it answers itself. Add it with
/// `inner_middleware` on [`Router::callback_query`](super::Router::callback_query), so it answers only presses a
/// handler takes. When both the handler and the answer fail, the handler's error is returned.
///
/// ```no_run
/// use vaegram::handlers::{CallbackAnswer, Dep, Router, answer_callback_queries};
/// use vaegram::prelude::*;
///
/// async fn saved(_query: CallbackQuery, Dep(answer): Dep<CallbackAnswer>) -> Result<()> {
///     answer.text("Saved");
///     Ok(())
/// }
///
/// let mut router = Router::new();
/// router.callback_query().inner_middleware(answer_callback_queries());
/// router.callback_query().handle(filters::data("save"), saved);
/// ```
pub fn answer_callback_queries() -> impl Fn(UpdateContext, Next) -> MiddlewareFuture + Clone + Send + Sync + 'static {
    |mut ctx: UpdateContext, next: Next| -> MiddlewareFuture {
        Box::pin(async move {
            let Some(query) = ctx.callback_query().cloned() else { return next.run(ctx).await };
            let answer = CallbackAnswer::default();
            ctx.provide(answer.clone());
            let result = next.run(ctx).await;
            let reply = std::mem::take(&mut *answer.0.lock().unwrap_or_else(PoisonError::into_inner));
            if reply.off {
                return result;
            }
            let mut request = query.answer();
            if let Some(text) = reply.text {
                request = request.text(text);
            }
            if reply.alert {
                request = request.alert();
            }
            if let Some(url) = reply.url {
                request = request.url(url);
            }
            let answered = request.await;
            result.and(answered)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::ChatAction;
    use crate::filters;
    use crate::handlers::Router;
    use crate::raw::{Deserializable, base};
    use crate::test::{TestChat, TestMessage, TestUser};

    /// The topic a recorded `messages.setTyping` shows the action in: its `top_msg_id`.
    fn typing_topic(query: &[u8]) -> Option<i32> {
        let buf = &mut query.get(4..)?;
        let flags = u32::deserialize(buf).ok()?;
        base::InputPeer::deserialize(buf).ok()?;
        if flags & 1 == 0 {
            return None;
        }
        i32::deserialize(buf).ok()
    }

    #[tokio::test]
    async fn chat_action_shows_in_the_messages_topic() {
        let (client, recorder) = Client::recording();
        let mut router = Router::new();
        router.inner_middleware(super::chat_action(ChatAction::Typing));
        router.on_message(filters::all(), |_| async { Ok(()) });
        client.include_router(router);

        let forum = TestChat::supergroup(5).forum(true);
        recorder.feed(TestMessage::in_chat(forum, TestUser::new(42), "hi").thread_id(10)).await;
        recorder.feed(TestMessage::private(42, "hi")).await;
        let topics: Vec<_> = recorder
            .calls()
            .iter()
            .filter(|call| call.method == "messages.setTyping")
            .map(|call| typing_topic(call.query()))
            .collect();
        assert_eq!(topics, [Some(10), None]);
    }
}
