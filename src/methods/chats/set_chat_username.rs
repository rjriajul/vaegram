use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Sets the public username of a channel or supergroup. `None` makes it private.
    pub async fn set_chat_username(&self, chat_id: impl Into<PeerRef>, username: Option<&str>) -> Result<()> {
        let channel = self.resolve_channel(&chat_id.into()).await?;
        let username = username.unwrap_or_default().to_owned();
        self.invoke(&functions::channels::UpdateUsername { channel, username }).await?;
        Ok(())
    }
}
