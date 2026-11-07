use std::future::Future;

use crate::Client;
use crate::errors::Result;

impl Client {
    /// [`run`](Self::run) until Ctrl-C: the first Ctrl-C calls [`stop`](Self::stop), so `run` stops taking updates,
    /// waits for the handlers already running and this returns `Ok(())`. It returns `Error::Disconnected` when the
    /// connection ends first, like `run`.
    ///
    /// It watches Ctrl-C only (SIGINT on Unix), not SIGTERM, which containers and service managers send to stop a
    /// process; for that, wait for the signal yourself and call `stop`.
    ///
    /// Once Ctrl-C is watched, tokio keeps it from ending the process for as long as the process runs, so a
    /// second Ctrl-C while handlers finish does nothing. If Ctrl-C can't be watched, this is just `run`.
    pub async fn run_until_ctrl_c(&self) -> Result<()> {
        self.run_until(async {
            if tokio::signal::ctrl_c().await.is_err() {
                std::future::pending::<()>().await;
            }
        })
        .await
    }

    /// `run`, with `stop` called once `signal` resolves.
    pub(super) async fn run_until(&self, signal: impl Future<Output = ()>) -> Result<()> {
        let mut run = std::pin::pin!(self.run());
        tokio::select! {
            result = &mut run => return result,
            () = signal => self.stop(),
        }
        run.await
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::sync::oneshot;

    use crate::Client;
    use crate::raw::{Serializable, base, types};
    use crate::session::UpdatesRx;
    use crate::storage::UpdateState;

    #[tokio::test]
    async fn the_signal_stops_run() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let empty = base::updates::Difference::DifferenceEmpty(types::updates::DifferenceEmpty { date: 2, seq: 3 });
        let (client, _sent) = Client::scripted(vec![Ok(me.to_bytes()), Ok(empty.to_bytes())]);
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 10, date: 1, ..Default::default() });
        // Updates never arrive and the channel stays open, so only the signal can end run.
        let (_updates, rx) = UpdatesRx::channel();
        *client.0.updates.lock().await = rx;

        let (fire, signal) = oneshot::channel::<()>();
        let running = client.clone();
        let mut task = tokio::spawn(async move {
            running
                .run_until(async {
                    let _ = signal.await;
                })
                .await
        });
        assert!(tokio::time::timeout(Duration::from_millis(200), &mut task).await.is_err(), "run ended on its own");
        assert!(!*client.0.stop.borrow());

        fire.send(()).unwrap();
        let result = tokio::time::timeout(Duration::from_secs(10), task).await.expect("the signal didn't stop run");
        assert!(matches!(result, Ok(Ok(()))), "{result:?}");
    }
}
