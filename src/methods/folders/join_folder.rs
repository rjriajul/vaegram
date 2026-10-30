use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

use super::check_folder_invite_link::slug;

impl Client {
    pub async fn join_folder(&self, invite_link: &str) -> Result<()> {
        let slug = slug(invite_link)?.to_owned();
        let info = self.check_folder_invite_link(invite_link).await?;
        let peers = self.resolve_peers(info.missing_chats.iter().map(|c| c.id)).await?;
        self.invoke(&functions::chatlists::JoinChatlistInvite { slug, peers }).await?;
        Ok(())
    }
}
