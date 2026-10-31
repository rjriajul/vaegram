use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PeerRef;

impl Client {
    pub async fn get_chat_invite_link_joiners_count(
        &self,
        chat_id: impl Into<PeerRef>,
        invite_link: impl Into<String>,
    ) -> Result<i32> {
        let request = functions::messages::GetChatInviteImporters {
            requested: false,
            subscription_expired: false,
            peer: self.resolve_peer(chat_id).await?,
            link: Some(invite_link.into()),
            q: None,
            offset_date: 0,
            offset_user: base::InputUser::InputUserEmpty,
            limit: 1,
        };
        let base::messages::ChatInviteImporters::ChatInviteImporters(page) = self.invoke(&request).await?;
        Ok(page.count)
    }
}
