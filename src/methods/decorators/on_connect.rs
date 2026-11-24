use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::handlers::{HandlerRef, UpdateKind, callback};

impl Client {
    /// Runs when the main connection is back after it was lost or `restart`ed. The first connection is made
    /// before a hook can be registered, so it never sees that one. It runs in a task of its own, so it has no
    /// order with `on_disconnect`; file connections don't trigger it. An `Err` goes to `on_error`. Registering
    /// again replaces it.
    pub fn on_connect<F, Fut, T>(&self, f: F) -> HandlerRef<'_>
    where
        F: Fn(Client) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.watch_link();
        self.register(UpdateKind::Connect, |r, id| r.hooks.set(UpdateKind::Connect, id, callback(f)))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::time::Duration;

    use tokio::sync::mpsc;

    use crate::errors::Error;
    use crate::handlers::UpdateKind;
    use crate::session::test_dc::TestDc;

    const GUARD: Duration = Duration::from_secs(10);

    #[tokio::test]
    async fn restart_runs_both_connect_hooks() {
        let dc = TestDc::start([5; 256], false).await;
        let client = dc.client([5; 256]).await;
        let (tx, mut seen) = mpsc::unbounded_channel();
        let down = tx.clone();
        client.on_connect(move |_| {
            let _ = tx.send("connect");
            async { Ok(()) }
        });
        client.on_disconnect(move |_| {
            let _ = down.send("disconnect");
            async { Ok(()) }
        });

        client.restart().unwrap();
        let mut both = HashSet::new();
        while both.len() < 2 {
            both.insert(tokio::time::timeout(GUARD, seen.recv()).await.unwrap().unwrap());
        }
        assert_eq!(both, HashSet::from(["connect", "disconnect"]));
    }

    #[tokio::test]
    async fn a_failing_connect_hook_reaches_on_error() {
        let dc = TestDc::start([5; 256], false).await;
        let client = dc.client([5; 256]).await;
        client.on_connect(|_| async { Err::<(), _>(Error::Unexpected("boom")) });
        let (tx, mut failed) = mpsc::unbounded_channel();
        client.on_error(move |e| {
            let _ = tx.send((e.kind, e.chat_id));
            async {}
        });

        client.restart().unwrap();
        let failure = tokio::time::timeout(GUARD, failed.recv()).await.unwrap();
        assert_eq!(failure, Some((UpdateKind::Connect, None)));
    }
}
