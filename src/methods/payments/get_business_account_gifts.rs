use crate::Client;
use crate::methods::GetChatGifts;

impl Client {
    pub fn get_business_account_gifts(&self, business_connection_id: impl Into<String>) -> GetChatGifts<'_> {
        GetChatGifts::new(self, None, Some(business_connection_id.into()))
    }
}
