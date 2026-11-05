use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Clears the list of recently used emoji statuses.
    pub async fn clear_recent_emoji_statuses(&self) -> Result<()> {
        self.invoke(&functions::account::ClearRecentEmojiStatuses {}).await?;
        Ok(())
    }
}
