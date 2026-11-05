use crate::Client;
use crate::methods::VerifyChat;
use crate::types::PeerRef;

impl Client {
    /// Verifies a user on behalf of your organization. Returns the same request as [`Client::verify_chat`].
    pub fn verify_user(&self, user_id: impl Into<PeerRef>) -> VerifyChat<'_> {
        self.verify_chat(user_id)
    }
}
