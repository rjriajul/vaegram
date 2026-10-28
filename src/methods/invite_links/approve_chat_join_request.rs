use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn approve_chat_join_request(
        &self,
        chat_id: impl Into<PeerRef>,
        user_id: impl Into<PeerRef>,
    ) -> Result<()> {
        self.hide_chat_join_request(chat_id.into(), user_id.into(), true).await
    }

    pub(super) async fn hide_chat_join_request(
        &self,
        chat_id: PeerRef,
        user_id: PeerRef,
        approved: bool,
    ) -> Result<()> {
        let request = functions::messages::HideChatJoinRequest {
            approved,
            peer: self.resolve_peer(chat_id).await?,
            user_id: self.resolve_user(&user_id).await?,
        };
        self.invoke(&request).await?;
        Ok(())
    }
}
