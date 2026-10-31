use std::future::{Future, IntoFuture};
use std::pin::Pin;

use super::Spec;
use crate::Client;
use crate::errors::Result;
use crate::handlers::Listenable;
use crate::types::Message;

#[must_use = "a request does nothing until it is awaited"]
pub struct StopListening<'a, T = Message> {
    client: &'a Client,
    spec: Spec<T>,
}

impl Client {
    /// Stops the listeners of kind `T` that the pattern covers and returns how many it stopped.
    ///
    /// Each criterion set here must share a value with the listener's own; a listener without that criterion
    /// isn't covered, and with no criteria every listener of `T` stops. Waiting listeners fail with
    /// `Error::ListenerStopped`; next-step handlers are removed.
    pub fn stop_listening<T: Listenable>(&self) -> StopListening<'_, T> {
        StopListening { client: self, spec: Spec::default() }
    }
}

listener_setters!(StopListening, pattern);

impl<T: Listenable> StopListening<'_, T> {
    async fn send(self) -> Result<usize> {
        let pattern = self.spec.resolve(self.client).await?;
        Ok(T::book(&self.client.0.handlers.listeners).stop(&pattern))
    }
}

impl<'a, T: Listenable> IntoFuture for StopListening<'a, T> {
    type Output = Result<usize>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::dispatcher::listeners::tests::entries;
    use crate::errors::{Error, Result};
    use crate::types::{CallbackQuery, Message};

    fn spawn_wait(client: &Client, chat: Option<i64>) -> tokio::task::JoinHandle<Result<Message>> {
        let client = client.clone();
        tokio::spawn(async move {
            match chat {
                Some(chat) => client.wait_for_message(chat).await,
                None => client.listen::<Message>().await,
            }
        })
    }

    async fn registered(client: &Client, count: usize) {
        for _ in 0..1000 {
            if entries(&client.0.handlers.listeners.messages) == count {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("the listeners never registered");
    }

    #[tokio::test]
    async fn stop_listening_counts_and_fails_waiters() {
        let client = Client::offline();
        let (first, second, other) =
            (spawn_wait(&client, Some(7)), spawn_wait(&client, Some(7)), spawn_wait(&client, Some(8)));
        registered(&client, 3).await;
        assert_eq!(client.stop_listening::<Message>().chat(7).await.unwrap(), 2);
        for waiter in [first, second] {
            assert!(matches!(waiter.await.unwrap(), Err(Error::ListenerStopped)));
        }
        assert!(!other.is_finished());
        other.abort();
    }

    #[tokio::test]
    async fn an_empty_pattern_stops_every_listener_of_its_kind_only() {
        let client = Client::offline();
        let waiters = [spawn_wait(&client, Some(7)), spawn_wait(&client, None)];
        registered(&client, 2).await;
        client.register_next_step_handler(|_: CallbackQuery| async { Ok(()) }).await.unwrap();
        assert_eq!(client.stop_listening::<Message>().await.unwrap(), 2);
        for waiter in waiters {
            assert!(matches!(waiter.await.unwrap(), Err(Error::ListenerStopped)));
        }
        assert!(client.0.handlers.listeners.callbacks.waiting(), "a callback listener was stopped");
    }

    #[tokio::test]
    async fn a_chat_pattern_does_not_stop_chatless_listeners() {
        let client = Client::offline();
        let chatless = spawn_wait(&client, None);
        registered(&client, 1).await;
        assert_eq!(client.stop_listening::<Message>().chat(7).await.unwrap(), 0);
        assert!(!chatless.is_finished());
        chatless.abort();
    }

    #[tokio::test]
    async fn next_step_handlers_are_stopped_too() {
        let client = Client::offline();
        let id = client.register_next_step_handler(|_: Message| async { Ok(()) }).chat(7).await.unwrap();
        assert_eq!(client.stop_listening::<Message>().chat(7).await.unwrap(), 1);
        assert!(!client.0.handlers.listeners.messages.waiting());
        assert!(!client.remove_handler(id));
    }
}
