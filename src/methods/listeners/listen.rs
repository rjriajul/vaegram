use std::future::{Future, IntoFuture};
use std::pin::Pin;

use super::Spec;
use crate::Client;
use crate::errors::Result;
use crate::handlers::Listenable;
use crate::types::Message;

#[must_use = "a listener does nothing until it is awaited"]
pub struct Listen<'a, T = Message> {
    pub(super) client: &'a Client,
    pub(super) spec: Spec<T>,
}

impl Client {
    /// Waits for the next update of kind `T` (`Message` unless turbofished) that matches every criterion set.
    ///
    /// The update this listener takes skips every handler group; raw handlers still see it. When several
    /// listeners match, the oldest takes it. It waits for the client's `listener_timeout` (5 minutes unless set)
    /// unless `timeout` or `no_timeout` says otherwise, then fails with `Error::ListenerTimeout`. Service messages
    /// count as messages: add `filters::text()` to wait for a text answer.
    ///
    /// Fails with `Error::ListenerStopped` between runs and when `run` stops. A handler waiting here keeps its
    /// `max_concurrent_handlers` slot.
    pub fn listen<T: Listenable>(&self) -> Listen<'_, T> {
        Listen { client: self, spec: Spec::new(self) }
    }
}

listener_setters!(Listen);

impl<T: Listenable> Listen<'_, T> {
    async fn send(self) -> Result<T> {
        let criteria = self.spec.resolve(self.client).await?;
        let listeners = &self.client.0.handlers.listeners;
        Ok(T::book(listeners).listen(listeners.next_id(), criteria).await?.prepare().await)
    }
}

impl<'a, T: Listenable> IntoFuture for Listen<'a, T> {
    type Output = Result<T>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

#[cfg(test)]
mod tests {
    use std::future::IntoFuture;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use crate::Client;
    use crate::dispatcher::listeners::Offer;
    use crate::dispatcher::listeners::tests::{dispatch, message, raw_query, until_waiting};
    use crate::errors::Error;
    use crate::filters;
    use crate::raw::{Serializable, base, types};
    use crate::session::UpdatesRx;
    use crate::storage::UpdateState;
    use crate::types::{CallbackQuery, Message};

    #[tokio::test(start_paused = true)]
    async fn listen_times_out_with_listener_timeout() {
        let client = Client::offline();
        let got = client.listen::<Message>().timeout(Duration::from_secs(1)).await;
        assert!(matches!(got, Err(Error::ListenerTimeout)), "{got:?}");
        assert!(!client.0.handlers.listeners.messages.waiting());
    }

    fn with_listener_timeout(timeout: Option<Duration>) -> Client {
        let mut client = Client::offline();
        Arc::get_mut(&mut client.0).unwrap().listener_timeout = timeout;
        client
    }

    #[tokio::test(start_paused = true)]
    async fn the_client_listener_timeout_is_the_default() {
        let client = with_listener_timeout(Some(Duration::from_secs(1)));
        let got = client.listen::<Message>().await;
        assert!(matches!(got, Err(Error::ListenerTimeout)), "{got:?}");
        assert!(!client.0.handlers.listeners.messages.waiting());
    }

    #[tokio::test(start_paused = true)]
    async fn a_per_call_timeout_beats_the_client_default() {
        let client = with_listener_timeout(Some(Duration::from_secs(1)));
        let waiter = tokio::spawn({
            let client = client.clone();
            async move { client.listen::<Message>().timeout(Duration::from_secs(10)).await }
        });
        let book = &client.0.handlers.listeners.messages;
        until_waiting(book).await;
        tokio::time::advance(Duration::from_secs(2)).await;
        let me = client.0.me.lock().unwrap().clone();
        assert!(matches!(book.offer(message(&client, 4, 7, None, false), &me), Offer::Claimed));
        assert_eq!(waiter.await.unwrap().unwrap().id, 4);

        let client = with_listener_timeout(None);
        let got = client.listen::<Message>().timeout(Duration::from_secs(1)).await;
        assert!(matches!(got, Err(Error::ListenerTimeout)), "{got:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn no_timeout_waits_past_the_default() {
        let client = Client::offline();
        let waiter = tokio::spawn({
            let client = client.clone();
            async move { client.listen::<Message>().no_timeout().await }
        });
        let book = &client.0.handlers.listeners.messages;
        until_waiting(book).await;
        tokio::time::advance(Duration::from_secs(301)).await;
        let me = client.0.me.lock().unwrap().clone();
        assert!(matches!(book.offer(message(&client, 4, 7, None, false), &me), Offer::Claimed));
        assert_eq!(waiter.await.unwrap().unwrap().id, 4);
    }

    #[tokio::test]
    async fn dropping_the_listen_future_removes_the_listener() {
        let client = Client::offline();
        let mut listen = client.listen::<Message>().into_future();
        tokio::select! {
            biased;
            _ = &mut listen => panic!("nothing was offered"),
            _ = std::future::ready(()) => {}
        }
        assert!(client.0.handlers.listeners.messages.waiting());
        drop(listen);
        assert!(!client.0.handlers.listeners.messages.waiting());
    }

    #[tokio::test]
    async fn listen_fails_at_once_between_runs() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let empty = base::updates::Difference::DifferenceEmpty(types::updates::DifferenceEmpty { date: 2, seq: 3 });
        let (client, _sent) = Client::scripted(vec![Ok(me.to_bytes()), Ok(empty.to_bytes())]);
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 10, date: 1, ..Default::default() });
        let (_updates, rx) = UpdatesRx::channel();
        *client.0.updates.lock().await = rx;
        client.stop();
        assert!(client.run().await.is_ok());
        let got = client.listen::<Message>().await;
        assert!(matches!(got, Err(Error::ListenerStopped)), "{got:?}");
    }

    #[tokio::test]
    async fn me_resolves_without_a_request() {
        let (client, mut sent) = Client::recording();
        client.0.me.lock().unwrap().id = 5;
        let mut listen = client.listen::<Message>().chat("me").user("me").into_future();
        tokio::select! {
            biased;
            _ = &mut listen => panic!("nothing was offered"),
            _ = std::future::ready(()) => {}
        }
        assert!(client.0.handlers.listeners.messages.waiting());
        assert!(sent.try_recv().is_err(), "a request was sent");
        let me = client.0.me.lock().unwrap().clone();
        let book = &client.0.handlers.listeners.messages;
        assert!(matches!(book.offer(message(&client, 1, 7, None, false), &me), Offer::Unclaimed(_)));
    }

    #[tokio::test]
    async fn no_client_alert_answers_no_strangers() {
        let (mut client, mut sent) = Client::recording();
        Arc::get_mut(&mut client.0).unwrap().unallowed_click_alert = None;
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        client.on_callback_query(filters::all(), move |q: CallbackQuery| {
            log.lock().unwrap().push(q.from_user.id);
            async { Ok(()) }
        });
        let waiter = tokio::spawn({
            let client = client.clone();
            async move { client.listen::<CallbackQuery>().user(8).message_id(2).await }
        });
        until_waiting(&client.0.handlers.listeners.callbacks).await;
        dispatch(&client, base::Update::UpdateBotCallbackQuery(Box::new(raw_query(9, 7, 2))));
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
        assert_eq!(*seen.lock().unwrap(), [9]);
        assert!(sent.try_recv().is_err(), "an answer was sent");
        assert!(!waiter.is_finished());
        waiter.abort();
    }
}
