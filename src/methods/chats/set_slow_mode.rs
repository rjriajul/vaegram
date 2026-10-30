use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn set_slow_mode(&self, chat_id: impl Into<PeerRef>, seconds: Option<i32>) -> Result<()> {
        let channel = self.resolve_channel(&chat_id.into()).await?;
        self.invoke(&functions::channels::ToggleSlowMode { channel, seconds: seconds.unwrap_or(0) }).await?;
        Ok(())
    }
}
