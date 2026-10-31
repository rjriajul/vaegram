use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};

#[must_use = "a request does nothing until it is awaited"]
pub struct Takeout<'a> {
    client: &'a Client,
    request: functions::account::InitTakeoutSession,
}

impl Client {
    /// Starts a data export session; pick what it covers with the setters, then await it. User accounts only.
    ///
    /// Telegram may answer `TAKEOUT_INIT_DELAY_<seconds>` until the user confirms the export in another app or
    /// the delay passes; [`Error::flood_wait`](crate::Error::flood_wait) gives the wait, and waits up to
    /// `flood_sleep_threshold` are slept off.
    pub fn takeout(&self) -> Takeout<'_> {
        let request = functions::account::InitTakeoutSession {
            contacts: false,
            message_users: false,
            message_chats: false,
            message_megagroups: false,
            message_channels: false,
            files: false,
            file_max_size: None,
        };
        Takeout { client: self, request }
    }
}

impl Takeout<'_> {
    pub fn contacts(mut self) -> Self {
        self.request.contacts = true;
        self
    }

    pub fn private_chats(mut self) -> Self {
        self.request.message_users = true;
        self
    }

    /// Basic groups that were upgraded continue as supergroups, so their later history needs `supergroups()`.
    pub fn basic_groups(mut self) -> Self {
        self.request.message_chats = true;
        self
    }

    pub fn supergroups(mut self) -> Self {
        self.request.message_megagroups = true;
        self
    }

    pub fn channels(mut self) -> Self {
        self.request.message_channels = true;
        self
    }

    /// Files up to `max_size` bytes.
    pub fn files(mut self, max_size: u64) -> Self {
        self.request.files = true;
        self.request.file_max_size = Some(i64::try_from(max_size).unwrap_or(i64::MAX));
        self
    }

    async fn send(self) -> Result<TakeoutSession> {
        // The init itself must not run inside another takeout session.
        let plain = Client(self.client.0.clone(), None);
        let base::account::Takeout::Takeout(takeout) = plain.invoke(&self.request).await?;
        Ok(TakeoutSession { client: Client(plain.0, Some(takeout.id)), finished: false })
    }
}

into_future!(Takeout, TakeoutSession);

/// An open data export session. Dropping it unfinished aborts it, as `finish(false)` does, in the background.
#[must_use = "an unfinished takeout session is aborted when dropped"]
#[derive(Debug)]
pub struct TakeoutSession {
    client: Client,
    finished: bool,
}

impl TakeoutSession {
    /// A client whose requests, downloads included, run inside this session. A clone used after the session
    /// ends gets `TAKEOUT_INVALID`.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Ends the session; `success = false` aborts it.
    pub async fn finish(mut self, success: bool) -> Result<bool> {
        self.finished = true;
        self.client.invoke(&functions::account::FinishTakeoutSession { success }).await
    }
}

impl Drop for TakeoutSession {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let client = self.client.clone();
            runtime.spawn(async move {
                let _ = client.invoke(&functions::account::FinishTakeoutSession { success: false }).await;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::sync::mpsc::UnboundedReceiver;

    use super::*;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Serializable, types};

    fn takeout_reply(id: i64) -> Result<Vec<u8>> {
        Ok(base::account::Takeout::Takeout(types::account::Takeout { id }).to_bytes())
    }

    async fn opened(mut replies: Vec<Result<Vec<u8>>>) -> (Client, TakeoutSession, UnboundedReceiver<Vec<u8>>) {
        replies.insert(0, takeout_reply(9));
        let (client, mut sent) = Client::scripted(replies);
        let session = client.takeout().contacts().await.unwrap();
        sent.try_recv().unwrap();
        (client, session, sent)
    }

    fn finish(success: bool) -> Vec<u8> {
        let query = functions::account::FinishTakeoutSession { success };
        functions::InvokeWithTakeout { takeout_id: 9, query }.to_bytes()
    }

    #[tokio::test]
    async fn takeout_sends_the_chosen_kinds() {
        let (client, mut sent) = Client::scripted(vec![takeout_reply(9)]);
        let takeout = client.takeout().contacts().private_chats().basic_groups().supergroups().channels().files(1024);
        let session = takeout.await.unwrap();
        let expected = functions::account::InitTakeoutSession {
            contacts: true,
            message_users: true,
            message_chats: true,
            message_megagroups: true,
            message_channels: true,
            files: true,
            file_max_size: Some(1024),
        };
        assert_eq!(sent.try_recv().unwrap(), expected.to_bytes());
        assert_eq!(session.client().1, Some(9));
    }

    #[tokio::test]
    async fn requests_inside_go_through_the_session() {
        let pong = Ok(types::Pong { msg_id: 0, ping_id: 7 }.to_bytes());
        let (_client, session, mut sent) = opened(vec![pong]).await;
        session.client().invoke(&functions::Ping { ping_id: 7 }).await.unwrap();
        let expected = functions::InvokeWithTakeout { takeout_id: 9, query: functions::Ping { ping_id: 7 } };
        assert_eq!(sent.try_recv().unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn finish_reports_success_inside_the_session() {
        let (_client, session, mut sent) = opened(vec![Ok(true.to_bytes())]).await;
        assert!(session.finish(true).await.unwrap());
        assert_eq!(sent.try_recv().unwrap(), finish(true));
    }

    #[tokio::test]
    async fn dropping_an_unfinished_session_aborts_it() {
        let (_client, session, mut sent) = opened(vec![Ok(true.to_bytes())]).await;
        drop(session);
        let body = tokio::time::timeout(Duration::from_secs(5), sent.recv()).await.expect("no abort sent");
        assert_eq!(body.unwrap(), finish(false));
    }

    #[tokio::test]
    async fn a_finished_session_is_not_aborted_on_drop() {
        let (client, session, mut sent) = opened(vec![Ok(true.to_bytes())]).await;
        session.finish(false).await.unwrap();
        assert_eq!(sent.try_recv().unwrap(), finish(false));
        // A spawned abort would hold its own handle to the client.
        assert_eq!(Arc::strong_count(&client.0), 1);
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn init_from_a_takeout_client_is_not_nested() {
        let (client, mut sent) = Client::scripted(vec![takeout_reply(9)]);
        let session = Client(client.0.clone(), Some(3)).takeout().contacts().await.unwrap();
        let expected = client.takeout().contacts().request;
        assert_eq!(sent.try_recv().unwrap(), expected.to_bytes());
        assert_eq!(session.client().1, Some(9));
    }

    #[test]
    fn files_size_saturates() {
        let client = Client::offline();
        assert_eq!(client.takeout().files(5).request.file_max_size, Some(5));
        assert_eq!(client.takeout().files(u64::MAX).request.file_max_size, Some(i64::MAX));
    }

    #[tokio::test(start_paused = true)]
    async fn a_long_init_delay_reaches_the_caller() {
        let delay = Err(Error::Rpc(RpcError::new(420, "TAKEOUT_INIT_DELAY_86400")));
        let (client, mut sent) = Client::scripted(vec![delay]);
        let e = client.takeout().contacts().await.unwrap_err();
        assert_eq!(e.flood_wait(), Some(Duration::from_secs(86400)));
        assert!(sent.try_recv().is_ok());
        assert!(sent.try_recv().is_err());
    }
}
