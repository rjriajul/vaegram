use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PeerRef;
use crate::utils::compute_password_check;

impl Client {
    pub async fn transfer_chat_ownership(
        &self,
        chat_id: impl Into<PeerRef>,
        user_id: impl Into<PeerRef>,
        password: &str,
    ) -> Result<()> {
        let peer = self.resolve_peer(chat_id).await?;
        let user_id = self.resolve_user(&user_id.into()).await?;
        let base::account::Password::Password(p) = self.invoke(&functions::account::GetPassword {}).await?;
        let request =
            functions::messages::EditChatCreator { peer, user_id, password: compute_password_check(&p, password)? };
        self.invoke(&request).await?;
        Ok(())
    }
}
