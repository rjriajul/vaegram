use std::sync::Arc;

use tokio::task::JoinSet;

use crate::Client;
use crate::errors::{Error, Result};

impl Client {
    /// Runs every client's `run` at once and returns once all of them have returned: `Ok(())`, or the first
    /// error. A client that fails doesn't stop the others; call `stop` on each to end it.
    ///
    /// Fails with `Error::InvalidArgument` before running anything when the same client is given twice.
    pub async fn compose(clients: &[Client]) -> Result<()> {
        for (i, client) in clients.iter().enumerate() {
            if clients[..i].iter().any(|other| Arc::ptr_eq(&other.0, &client.0)) {
                return Err(Error::InvalidArgument("the same client is given twice"));
            }
        }
        let mut runs = JoinSet::new();
        for client in clients {
            let client = client.clone();
            runs.spawn(async move { client.run().await });
        }
        let mut result = Ok(());
        while let Some(joined) = runs.join_next().await {
            match joined {
                Ok(Err(error)) if result.is_ok() => result = Err(error),
                Ok(_) => {}
                Err(error) => {
                    if let Ok(panic) = error.try_into_panic() {
                        std::panic::resume_unwind(panic);
                    }
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::methods::utilities::run::tests::stopped_client;

    fn failing_client() -> Client {
        Client::scripted(vec![Err(Error::Rpc(RpcError::new(401, "AUTH_KEY_UNREGISTERED")))]).0
    }

    #[tokio::test]
    async fn compose_returns_once_every_client_has_stopped() {
        let ((first, _first_updates), (second, _second_updates)) = (stopped_client().await, stopped_client().await);
        assert!(Client::compose(&[first, second]).await.is_ok());
    }

    #[tokio::test]
    async fn compose_reports_the_first_failure_after_all_return() {
        let (running, _updates) = stopped_client().await;
        running.0.stop.send_replace(false);
        let composed = tokio::spawn({
            let clients = [failing_client(), running.clone()];
            async move { Client::compose(&clients).await }
        });
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }
        assert!(!composed.is_finished(), "compose returned before every client did");
        running.stop();
        let result = composed.await.unwrap();
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "AUTH_KEY_UNREGISTERED"), "{result:?}");
    }

    #[tokio::test]
    async fn the_same_client_twice_is_refused() {
        let (client, mut sent) = Client::recording();
        let result = Client::compose(&[client.clone(), client]).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(sent.try_recv().is_err(), "a client ran");
    }

    #[tokio::test]
    async fn no_clients_is_ok() {
        assert!(Client::compose(&[]).await.is_ok());
    }
}
