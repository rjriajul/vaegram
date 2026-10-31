use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{ChatInviteLink, PeerRef};

impl Client {
    pub async fn get_chat_invite_link(
        &self,
        chat_id: impl Into<PeerRef>,
        invite_link: impl Into<String>,
    ) -> Result<ChatInviteLink> {
        let request = functions::messages::GetExportedChatInvite {
            peer: self.resolve_peer(chat_id).await?,
            link: invite_link.into(),
        };
        ChatInviteLink::parse_edited(self, self.invoke(&request).await?)
    }
}
