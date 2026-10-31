use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PeerRef;

impl Client {
    /// `None` unlinks the channel's discussion group.
    pub async fn set_chat_discussion_group(
        &self,
        chat_id: impl Into<PeerRef>,
        discussion_chat_id: Option<PeerRef>,
    ) -> Result<()> {
        let broadcast = self.resolve_channel(&chat_id.into()).await?;
        let group = match discussion_chat_id {
            Some(chat_id) => self.resolve_channel(&chat_id).await?,
            None => base::InputChannel::InputChannelEmpty,
        };
        self.invoke(&functions::channels::SetDiscussionGroup { broadcast, group }).await?;
        Ok(())
    }
}
