use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn toggle_view_forum_as_messages(&self, chat_id: impl Into<PeerRef>, enabled: bool) -> Result<()> {
        let channel = self.resolve_channel(&chat_id.into()).await?;
        self.invoke(&functions::channels::ToggleViewForumAsMessages { channel, enabled }).await?;
        Ok(())
    }
}
