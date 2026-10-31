use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::handlers::{HandlerRef, UpdateKind, callback};

impl Client {
    /// Runs when the main connection is lost, `restart`ed or closed by `disconnect`, once per outage. It runs in a
    /// task of its own, so it has no order with `on_connect`; file connections don't trigger it. An `Err` goes to
    /// `on_error`. Registering again replaces it.
    pub fn on_disconnect<F, Fut, T>(&self, f: F) -> HandlerRef<'_>
    where
        F: Fn(Client) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.watch_link();
        self.register(UpdateKind::Disconnect, |r, id| r.hooks.set(UpdateKind::Disconnect, id, callback(f)))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::sync::mpsc;

    use crate::session::test_dc::TestDc;

    #[tokio::test]
    async fn disconnect_runs_the_disconnect_hook() {
        let dc = TestDc::start([5; 256], false).await;
        let client = dc.client([5; 256]).await;
        let (tx, mut seen) = mpsc::unbounded_channel();
        client.on_disconnect(move |_| {
            let _ = tx.send(());
            async { Ok(()) }
        });

        client.disconnect();
        assert_eq!(tokio::time::timeout(Duration::from_secs(10), seen.recv()).await.unwrap(), Some(()));
    }
}
