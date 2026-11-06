use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    /// Removes the verification your organization gave to a user.
    pub async fn remove_user_verification(&self, user_id: impl Into<PeerRef>) -> Result<()> {
        self.set_custom_verification(user_id.into(), false, None).await
    }
}
