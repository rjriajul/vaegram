use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn clear_recent_stickers(&self) -> Result<()> {
        self.invoke(&functions::messages::ClearRecentStickers { attached: false }).await?;
        Ok(())
    }
}
