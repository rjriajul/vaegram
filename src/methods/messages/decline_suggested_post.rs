use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
/// Request to decline a suggested post. Await it to send.
pub struct DeclineSuggestedPost<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    comment: Option<String>,
}

impl Client {
    /// Declines the suggested post in `message_id`.
    pub fn decline_suggested_post(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> DeclineSuggestedPost<'_> {
        DeclineSuggestedPost { client: self, chat_id: chat_id.into(), message_id, comment: None }
    }
}

impl DeclineSuggestedPost<'_> {
    /// Attaches a reason for declining the post.
    pub fn comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }

    async fn send(self) -> Result<()> {
        let request = functions::messages::ToggleSuggestedPostApproval {
            reject: true,
            peer: self.client.resolve_peer(self.chat_id).await?,
            msg_id: self.message_id,
            schedule_date: None,
            reject_comment: self.comment,
        };
        self.client.invoke(&request).await?;
        Ok(())
    }
}

into_future!(DeclineSuggestedPost, ());
