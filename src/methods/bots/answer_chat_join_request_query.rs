use crate::Client;
use crate::enums::ChatJoinRequestResult;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn answer_chat_join_request_query(&self, query_id: i64, result: ChatJoinRequestResult) -> Result<bool> {
        self.invoke(&functions::bots::SetJoinChatResults { query_id, result: result.write() }).await
    }
}
