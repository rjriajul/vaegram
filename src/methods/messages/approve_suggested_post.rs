use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
pub struct ApproveSuggestedPost<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    send_date: Option<i32>,
}

impl Client {
    pub fn approve_suggested_post(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> ApproveSuggestedPost<'_> {
        ApproveSuggestedPost { client: self, chat_id: chat_id.into(), message_id, send_date: None }
    }
}

impl ApproveSuggestedPost<'_> {
    /// Unix time to publish the post at; without it the post goes out right away.
    pub fn send_date(mut self, date: i32) -> Self {
        self.send_date = Some(date);
        self
    }

    async fn send(self) -> Result<()> {
        let request = functions::messages::ToggleSuggestedPostApproval {
            reject: false,
            peer: self.client.resolve_peer(self.chat_id).await?,
            msg_id: self.message_id,
            schedule_date: self.send_date,
            reject_comment: None,
        };
        self.client.invoke(&request).await?;
        Ok(())
    }
}

into_future!(ApproveSuggestedPost, ());
