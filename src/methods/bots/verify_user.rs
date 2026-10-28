use crate::Client;
use crate::methods::VerifyChat;
use crate::types::PeerRef;

impl Client {
    pub fn verify_user(&self, user_id: impl Into<PeerRef>) -> VerifyChat<'_> {
        self.verify_chat(user_id)
    }
}
