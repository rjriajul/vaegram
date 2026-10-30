use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn unblock_user(&self, user_id: impl Into<PeerRef>) -> Result<()> {
        let id = self.resolve_peer(user_id).await?;
        self.invoke(&functions::contacts::Unblock { my_stories_from: false, id }).await?;
        Ok(())
    }
}
