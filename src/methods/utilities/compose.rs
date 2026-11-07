use std::sync::Arc;

use tokio::task::JoinSet;

use crate::Client;
use crate::errors::{Error, Result};

/// Several clients running together: see [`Client::compose`]. Awaiting it starts them.
#[must_use = "clients start only when this is awaited"]
pub struct Compose<'a> {
    clients: &'a [Client],
    sequential: bool,
}

impl Client {
    /// Runs every client's `run` at once and returns once all of them have returned: `Ok(())`, or the first
    /// error. A client that fails doesn't stop the others; call `stop` on each to end it.
    ///
    /// Fails with `Error::InvalidArgument` before running anything when the same client is given twice.
    pub fn compose(clients: &[Client]) -> Compose<'_> {
        Compose { clients, sequential: false }
    }
}

impl Compose<'_> {
    /// Starts the clients one after another, in the order given, before any of them takes updates: each one's
    /// start (`get_me`, the update state and the start hook) finishes before the next begins, so a start hook can
    /// rely on the clients before it. A client whose start fails is skipped and its error kept; the others run
    /// together as usual, and the first error is returned once they all have.
    pub fn sequential(mut self) -> Self {
        self.sequential = true;
        self
    }

    async fn send(self) -> Result<()> {
        let clients = self.clients;
        for (i, client) in clients.iter().enumerate() {
            if clients[..i].iter().any(|other| Arc::ptr_eq(&other.0, &client.0)) {
                return Err(Error::InvalidArgument("the same client is given twice"));
            }
        }
        let mut result = Ok(());
        let mut runs = JoinSet::new();
        if self.sequential {
            let mut started = Vec::with_capacity(clients.len());
            for client in clients {
                let prepared = match client.claim_run() {
                    Ok(running) => client.prepare().await.map(|dispatcher| dispatcher.map(|d| (d, running))),
                    Err(error) => Err(error),
                };
                match prepared {
                    Ok(Some((dispatcher, running))) => started.push((client.clone(), dispatcher, running)),
                    Ok(None) => {}
                    Err(error) if result.is_ok() => result = Err(error),
                    Err(_) => {}
                }
            }
            for (client, dispatcher, running) in started {
                runs.spawn(async move { client.drive(running, dispatcher).await });
            }
        } else {
            for client in clients {
                let client = client.clone();
                runs.spawn(async move { client.run().await });
            }
        }
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

into_future!(Compose, ());

#[cfg(test)]
mod tests {
    use std::future::ready;
    use std::sync::{Arc, Mutex};

    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::methods::utilities::run::tests::stopped_client;

    fn failing_client() -> Client {
        Client::scripted(vec![Err(Error::Rpc(RpcError::parse(401, "AUTH_KEY_UNREGISTERED")))]).0
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

    type Log = Arc<Mutex<Vec<&'static str>>>;

    fn log_to(client: &Client, log: &Log, start: &'static str, stop: &'static str) {
        let (on_start, on_stop) = (log.clone(), log.clone());
        client.on_start(move |_| {
            on_start.lock().unwrap().push(start);
            ready(Ok(()))
        });
        client.on_stop(move |_| {
            on_stop.lock().unwrap().push(stop);
            ready(Ok(()))
        });
    }

    #[tokio::test]
    async fn sequential_prepares_in_order() {
        let ((first, _first_updates), (second, _second_updates)) = (stopped_client().await, stopped_client().await);
        let log = Log::default();
        log_to(&first, &log, "start first", "stop first");
        log_to(&second, &log, "start second", "stop second");
        assert!(Client::compose(&[first, second]).sequential().await.is_ok());
        let log = log.lock().unwrap();
        assert_eq!(log[..2], ["start first", "start second"], "{log:?}");
        assert_eq!(log.len(), 4, "{log:?}");
    }

    #[tokio::test]
    async fn sequential_skips_a_client_that_fails_to_start() {
        let (running, _updates) = stopped_client().await;
        let log = Log::default();
        log_to(&running, &log, "start", "stop");
        let result = Client::compose(&[failing_client(), running]).sequential().await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "AUTH_KEY_UNREGISTERED"), "{result:?}");
        assert_eq!(*log.lock().unwrap(), ["start", "stop"], "the other client didn't run");
    }

    #[tokio::test]
    async fn sequential_skips_a_client_disconnected_during_its_start_without_an_error() {
        // `disconnect` stops the client, then fails the start's waiting request with `Disconnected`.
        let (disconnected, _sent) = Client::scripted(vec![Err(Error::Disconnected)]);
        disconnected.disconnect();
        let (running, _updates) = stopped_client().await;
        let log = Log::default();
        log_to(&running, &log, "start", "stop");
        let result = Client::compose(&[disconnected, running]).sequential().await;
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(*log.lock().unwrap(), ["start", "stop"], "the other client didn't run");
    }
}
