use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn approve_all_chat_join_requests(
        &self,
        chat_id: impl Into<PeerRef>,
        invite_link: Option<&str>,
    ) -> Result<()> {
        self.hide_all_chat_join_requests(chat_id.into(), invite_link, true).await
    }

    pub(super) async fn hide_all_chat_join_requests(
        &self,
        chat_id: PeerRef,
        invite_link: Option<&str>,
        approved: bool,
    ) -> Result<()> {
        let request = functions::messages::HideAllChatJoinRequests {
            approved,
            peer: self.resolve_peer(chat_id).await?,
            link: invite_link.map(str::to_owned),
        };
        self.invoke(&request).await?;
        Ok(())
    }
}
