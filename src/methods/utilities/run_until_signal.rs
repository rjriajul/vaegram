use crate::Client;
use crate::errors::Result;

impl Client {
    /// [`run`](Self::run) until Ctrl-C or, on Unix, SIGTERM: the first signal calls [`stop`](Self::stop), so `run`
    /// stops taking updates, waits for the handlers already running and this returns `Ok(())`. It returns
    /// `Error::Disconnected` when the connection ends first, like `run`.
    ///
    /// SIGTERM is what containers and service managers (`docker stop`, systemd) send to stop a process, so a
    /// service should await this rather than [`run_until_ctrl_c`](Self::run_until_ctrl_c), which watches Ctrl-C
    /// only.
    ///
    /// SIGTERM is watched before `run` starts, so one that arrives early still stops it instead of ending the
    /// process. Once a signal is watched, tokio keeps it from ending the process for as long as the process runs, so
    /// a second signal while handlers finish does nothing. If SIGTERM can't be watched, this falls back to Ctrl-C;
    /// if Ctrl-C can't be watched either, it is just `run`.
    pub async fn run_until_signal(&self) -> Result<()> {
        // Created before `run_until` is entered, so a SIGTERM that arrives while `run` starts is caught.
        #[cfg(unix)]
        let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
        self.run_until(async move {
            #[cfg(unix)]
            {
                match terminate.as_mut() {
                    Some(terminate) => tokio::select! {
                        _ = ctrl_c() => {}
                        _ = terminate.recv() => {}
                    },
                    None => ctrl_c().await,
                }
            }
            #[cfg(not(unix))]
            {
                ctrl_c().await;
            }
        })
        .await
    }
}

/// Waits for Ctrl-C; if it can't be watched, waits for ever, so the caller is just `run`.
async fn ctrl_c() {
    if tokio::signal::ctrl_c().await.is_err() {
        std::future::pending::<()>().await;
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::time::Duration;

    use crate::Client;
    use crate::raw::{Serializable, base, types};
    use crate::session::UpdatesRx;
    use crate::storage::UpdateState;

    #[tokio::test]
    async fn sigterm_stops_run_until_signal() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let empty = base::updates::Difference::DifferenceEmpty(types::updates::DifferenceEmpty { date: 2, seq: 3 });
        let (client, _sent) = Client::scripted(vec![Ok(me.to_bytes()), Ok(empty.to_bytes())]);
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 10, date: 1, ..Default::default() });
        // Updates never arrive and the channel stays open, so only the signal can end run.
        let (_updates, rx) = UpdatesRx::channel();
        *client.0.updates.lock().await = rx;

        let running = client.clone();
        let mut task = tokio::spawn(async move { running.run_until_signal().await });
        assert!(tokio::time::timeout(Duration::from_millis(200), &mut task).await.is_err(), "run ended on its own");
        assert!(!*client.0.stop.borrow());

        // The SIGTERM handler is registered by now, so this stops `run` instead of the process.
        // SAFETY: raise only sends a signal to this thread; nothing here holds a pointer.
        assert_eq!(unsafe { libc::raise(libc::SIGTERM) }, 0);
        let result = tokio::time::timeout(Duration::from_secs(10), task).await.expect("SIGTERM didn't stop run");
        assert!(matches!(result, Ok(Ok(()))), "{result:?}");
    }
}
