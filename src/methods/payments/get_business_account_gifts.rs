use crate::Client;
use crate::methods::GetChatGifts;

impl Client {
    /// Lists the gifts of the business account behind this connection.
    ///
    /// Takes the same filters as [`Client::get_chat_gifts`]. Awaiting returns one page of
    /// [`OwnedGifts`](crate::types::OwnedGifts).
    pub fn get_business_account_gifts(&self, business_connection_id: impl Into<String>) -> GetChatGifts<'_> {
        GetChatGifts::new(self, None, Some(business_connection_id.into()))
    }
}
