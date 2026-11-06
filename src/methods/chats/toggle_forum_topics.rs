use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Turns topics on or off for a supergroup.
    ///
    /// `tabs` shows the topics as tabs instead of a list. `sidebar` shows the topics in a sidebar.
    pub async fn toggle_forum_topics(
        &self,
        chat_id: impl Into<PeerRef>,
        enabled: bool,
        tabs: bool,
        sidebar: bool,
    ) -> Result<()> {
        let channel = self.resolve_channel(&chat_id.into()).await?;
        self.invoke(&functions::channels::ToggleForum { channel, enabled, tabs, sidebar }).await?;
        Ok(())
    }
}
