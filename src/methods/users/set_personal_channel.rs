use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PeerRef;

impl Client {
    pub async fn set_personal_channel(&self, chat_id: Option<PeerRef>) -> Result<bool> {
        let channel = match chat_id {
            Some(chat_id) => self.resolve_channel(&chat_id).await?,
            None => base::InputChannel::InputChannelEmpty,
        };
        self.invoke(&functions::account::UpdatePersonalChannel { channel }).await
    }
}
