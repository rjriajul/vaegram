use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    pub async fn decline_all_chat_join_requests(
        &self,
        chat_id: impl Into<PeerRef>,
        invite_link: Option<&str>,
    ) -> Result<()> {
        self.hide_all_chat_join_requests(chat_id.into(), invite_link, false).await
    }
}
