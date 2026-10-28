use crate::Client;
use crate::methods::GetChatGifts;
use crate::types::PeerRef;

impl Client {
    pub fn get_user_gifts(&self, user_id: impl Into<PeerRef>) -> GetChatGifts<'_> {
        GetChatGifts::new(self, Some(user_id.into()), None)
    }
}
