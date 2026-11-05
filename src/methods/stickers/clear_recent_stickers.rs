use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Empties the account's recent stickers.
    pub async fn clear_recent_stickers(&self) -> Result<()> {
        self.invoke(&functions::messages::ClearRecentStickers { attached: false }).await?;
        Ok(())
    }
}
