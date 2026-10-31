use std::future::{Future, IntoFuture};
use std::pin::Pin;

use super::Spec;
use crate::Client;
use crate::errors::Result;
use crate::handlers::{Callback, HandlerId, Listenable, Registered, callback};
use crate::types::Message;

#[must_use = "a listener does nothing until it is awaited"]
pub struct RegisterNextStepHandler<'a, T = Message> {
    client: &'a Client,
    spec: Spec<T>,
    callback: Callback<T>,
}

impl Client {
    /// Runs `f` once, on the next update of kind `T` that matches, instead of waiting for it in place.
    ///
    /// Awaiting registers it and returns at once. The update it takes skips every handler group, as with
    /// `listen`, and `f` runs through the handler pipeline: `on_error`, `ordered` and `max_concurrent_handlers` apply.
    /// It expires silently after its timeout (5 minutes unless set); `remove_handler` removes it sooner. Fails
    /// with `Error::ListenerStopped` between runs.
    pub fn register_next_step_handler<T, F, Fut, R>(&self, f: F) -> RegisterNextStepHandler<'_, T>
    where
        T: Listenable,
        F: Fn(T) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<R>> + Send + 'static,
    {
        RegisterNextStepHandler { client: self, spec: Spec::default(), callback: callback(f) }
    }
}

listener_setters!(RegisterNextStepHandler);

impl<T: Listenable> RegisterNextStepHandler<'_, T> {
    async fn send(self) -> Result<HandlerId> {
        let criteria = self.spec.resolve(self.client).await?;
        let listeners = &self.client.0.handlers.listeners;
        let id = listeners.next_id();
        T::book(listeners).call(id, criteria, self.callback)?;
        Ok(HandlerId(Registered::Listener(id)))
    }
}

impl<'a, T: Listenable> IntoFuture for RegisterNextStepHandler<'a, T> {
    type Output = Result<HandlerId>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use crate::Client;
    use crate::dispatcher::listeners::tests::{dispatch, new_message, raw_query};
    use crate::errors::{Error, Result};
    use crate::handlers::UpdateKind;
    use crate::raw::base;
    use crate::types::{CallbackQuery, Message};

    type Log = Arc<Mutex<Vec<i32>>>;

    fn record(log: &Log) -> impl Fn(Message) -> std::future::Ready<Result<()>> + Send + Sync + 'static {
        let log = log.clone();
        move |m| {
            log.lock().unwrap().push(m.id);
            std::future::ready(Ok(()))
        }
    }

    async fn settle() {
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test]
    async fn runs_once_on_the_next_match() {
        let (client, log) = (Client::offline(), Log::default());
        client.register_next_step_handler(record(&log)).chat(7).await.unwrap();
        dispatch(&client, new_message(1, 8));
        dispatch(&client, new_message(2, 7));
        dispatch(&client, new_message(3, 7));
        settle().await;
        assert_eq!(*log.lock().unwrap(), [2]);
        assert!(!client.0.handlers.listeners.messages.waiting());
    }

    #[tokio::test]
    async fn errors_reach_on_error_with_the_kind() {
        let client = Client::offline();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        client.on_error(move |e| {
            log.lock().unwrap().push((e.kind, e.chat_id, matches!(e.error, Error::Unexpected("step"))));
            async {}
        });
        client
            .register_next_step_handler(|_: Message| async { Err::<(), _>(Error::Unexpected("step")) })
            .await
            .unwrap();
        let failing = |_: CallbackQuery| async { Err::<(), _>(Error::Unexpected("step")) };
        client.register_next_step_handler(failing).await.unwrap();
        dispatch(&client, new_message(1, 7));
        dispatch(&client, base::Update::UpdateBotCallbackQuery(Box::new(raw_query(7, 7, 2))));
        settle().await;
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2, "{seen:?}");
        assert!(seen.contains(&(UpdateKind::Message, Some(7), true)), "{seen:?}");
        assert!(seen.contains(&(UpdateKind::CallbackQuery, Some(7), true)), "{seen:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn an_expired_next_step_never_runs() {
        let (client, log) = (Client::offline(), Log::default());
        client.register_next_step_handler(record(&log)).timeout(Duration::from_secs(1)).await.unwrap();
        tokio::time::advance(Duration::from_secs(2)).await;
        dispatch(&client, new_message(1, 7));
        settle().await;
        assert!(log.lock().unwrap().is_empty());
        assert!(!client.0.handlers.listeners.messages.waiting(), "the expired entry was kept");
    }

    #[tokio::test]
    async fn registration_fails_while_closed() {
        let (client, log) = (Client::offline(), Log::default());
        client.0.handlers.listeners.close();
        let got = client.register_next_step_handler(record(&log)).await;
        assert!(matches!(got, Err(Error::ListenerStopped)), "{got:?}");
    }
}
