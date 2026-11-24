use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::session::Affected;
use crate::types::PeerId;
use crate::types::PeerRef;

/// Request built by [`Client::delete_messages`].
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteMessages<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_ids: Vec<i32>,
    revoke: bool,
}

impl Client {
    /// Deletes messages, by default for everyone.
    ///
    /// Takes 1 to 100 ids, otherwise awaiting returns `Error::InvalidArgument`. The deleted messages are also
    /// dropped from the client's message cache. Call [`revoke(false)`](DeleteMessages::revoke) to delete only
    /// for this account.
    ///
    /// ```no_run
    /// # async fn f(client: &vaegram::Client) -> vaegram::Result<()> {
    /// client.delete_messages(-1001234567890, &[10, 11]).await?;
    /// # Ok(()) }
    /// ```
    pub fn delete_messages(&self, chat_id: impl Into<PeerRef>, message_ids: &[i32]) -> DeleteMessages<'_> {
        DeleteMessages { client: self, chat_id: chat_id.into(), message_ids: message_ids.to_vec(), revoke: true }
    }
}

impl DeleteMessages<'_> {
    /// Whether to delete the messages for everyone in the chat (the default) or only for this account. Channels
    /// and supergroups have no "for me" delete: their messages always go for everyone, whatever this is.
    pub fn revoke(mut self, revoke: bool) -> Self {
        self.revoke = revoke;
        self
    }

    async fn send(self) -> Result<()> {
        let Self { client, chat_id, message_ids, revoke } = self;
        if !(1..=100).contains(&message_ids.len()) {
            return Err(Error::InvalidArgument("deletes take 1 to 100 message ids"));
        }
        let chat_id = client.peer_id(chat_id).await?;
        let id = message_ids.clone();
        let (channel_id, affected) = if let Some(channel_id) = PeerId::from_bot_api(chat_id).channel() {
            let channel = client.resolve_channel(&chat_id.into()).await?;
            (Some(channel_id), client.invoke(&functions::channels::DeleteMessages { channel, id }).await?)
        } else {
            (None, client.invoke(&functions::messages::DeleteMessages { revoke, id }).await?)
        };
        let base::messages::AffectedMessages::AffectedMessages(a) = affected;
        client.0.events.affected(Affected { channel_id, pts: a.pts, pts_count: a.pts_count });
        client.0.caches.evict(chat_id, &message_ids);
        Ok(())
    }
}

into_future!(DeleteMessages, ());

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, functions};

    #[tokio::test]
    async fn revoke_defaults_to_everyone_and_can_be_turned_off() {
        let (client, mut sent) = Client::recording();
        let _ = client.delete_messages(7, &[4]).await;
        let both = functions::messages::DeleteMessages { revoke: true, id: vec![4] };
        assert_eq!(sent.recv().await.unwrap(), both.to_bytes());

        let _ = client.delete_messages(7, &[4]).revoke(false).await;
        let mine = functions::messages::DeleteMessages { revoke: false, id: vec![4] };
        assert_eq!(sent.recv().await.unwrap(), mine.to_bytes());
    }
}
