use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;
use crate::utils::CHANNEL_OFFSET;

#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteChatHistory<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    max_id: i32,
    revoke: bool,
    just_clear: bool,
}

impl Client {
    pub fn delete_chat_history(&self, chat_id: impl Into<PeerRef>) -> DeleteChatHistory<'_> {
        DeleteChatHistory { client: self, chat_id: chat_id.into(), max_id: 0, revoke: false, just_clear: false }
    }
}

impl DeleteChatHistory<'_> {
    pub fn max_id(mut self, max_id: i32) -> Self {
        self.max_id = max_id;
        self
    }

    pub fn revoke(mut self) -> Self {
        self.revoke = true;
        self
    }

    pub fn just_clear(mut self) -> Self {
        self.just_clear = true;
        self
    }
}

into_future!(DeleteChatHistory, ());

impl DeleteChatHistory<'_> {
    async fn send(self) -> Result<()> {
        let client = self.client;
        let (chat_id, peer) = client.resolve(&self.chat_id).await?;
        if chat_id <= -CHANNEL_OFFSET {
            let channel = client.resolve_channel(&chat_id.into()).await?;
            let request =
                functions::channels::DeleteHistory { for_everyone: self.revoke, channel, max_id: self.max_id };
            client.invoke(&request).await?;
            return Ok(());
        }
        let request = functions::messages::DeleteHistory {
            just_clear: self.just_clear,
            revoke: self.revoke,
            peer,
            max_id: self.max_id,
            min_date: None,
            max_date: None,
        };
        client.affected_history(chat_id, &request).await
    }
}
