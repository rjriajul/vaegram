use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn delete_channel(&self, chat_id: impl Into<PeerRef>) -> Result<()> {
        let channel = self.resolve_channel(&chat_id.into()).await?;
        self.invoke(&functions::channels::DeleteChannel { channel }).await?;
        Ok(())
    }
}
