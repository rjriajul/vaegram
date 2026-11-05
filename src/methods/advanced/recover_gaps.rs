use tokio::sync::oneshot;

use crate::Client;
use crate::dispatcher::Recover;
use crate::errors::{Error, Result};

/// Asks Telegram for the updates this client missed, now rather than when it next notices a gap.
#[must_use = "a request does nothing until it is awaited"]
pub struct RecoverGaps<'a> {
    client: &'a Client,
    chat_ids: Option<Vec<i64>>,
}

impl Client {
    /// Fetches the updates this client missed, without waiting for it to notice a gap.
    ///
    /// Needs `run` to be taking updates, or fails with `Error::InvalidArgument`. Resolves to how many messages and then
    /// how many other updates came back, and they reach handlers as usual. The counts include whatever else was
    /// already due at that moment. A failed difference is reported as `ClientEvent::DifferenceFailed`, not here.
    pub fn recover_gaps(&self) -> RecoverGaps<'_> {
        RecoverGaps { client: self, chat_ids: None }
    }
}

impl RecoverGaps<'_> {
    /// Recovers only these: `0` for the common update box, `-100...` ids for channels.
    ///
    /// A channel this client has no update state for is skipped. By default everything is recovered.
    pub fn chat_ids(mut self, ids: impl IntoIterator<Item = i64>) -> Self {
        self.chat_ids = Some(ids.into_iter().collect());
        self
    }

    async fn send(self) -> Result<(usize, usize)> {
        let sender = self.client.0.recover.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        let sender = sender.ok_or(Error::InvalidArgument("recover_gaps needs run"))?;
        let (reply, answer) = oneshot::channel();
        sender.send(Recover { chats: self.chat_ids, reply }).map_err(|_| Error::Disconnected)?;
        answer.await.map_err(|_| Error::Disconnected)
    }
}

into_future!(RecoverGaps, (usize, usize));

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::dispatcher::Recover;
    use crate::errors::Error;

    #[tokio::test]
    async fn it_needs_run() {
        let result = Client::offline().recover_gaps().await;
        assert!(matches!(result, Err(Error::InvalidArgument(m)) if m.contains("run")), "{result:?}");
    }

    #[tokio::test]
    async fn a_dispatcher_that_goes_away_gives_disconnected() {
        let client = Client::offline();
        let (requests, mut inbox) = tokio::sync::mpsc::unbounded_channel::<Recover>();
        *client.0.recover.lock().unwrap() = Some(requests);
        let taker = tokio::spawn(async move {
            // Takes the request and drops it without answering, as a dispatcher that stops does.
            let request = inbox.recv().await.unwrap();
            assert_eq!(request.chats, Some(vec![0, -1001]));
        });
        let result = client.recover_gaps().chat_ids([0, -1001]).await;
        taker.await.unwrap();
        assert!(matches!(result, Err(Error::Disconnected)), "{result:?}");
        // With nobody receiving, sending fails the same way.
        let result = client.recover_gaps().await;
        assert!(matches!(result, Err(Error::Disconnected)), "{result:?}");
    }
}
