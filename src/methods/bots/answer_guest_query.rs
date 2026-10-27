use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InlineQueryResultArticle, SentGuestMessage};

impl Client {
    pub async fn answer_guest_query(
        &self,
        guest_query_id: i64,
        result: InlineQueryResultArticle,
    ) -> Result<SentGuestMessage> {
        let request =
            functions::messages::SetBotGuestChatResult { query_id: guest_query_id, result: result.write(self).await? };
        Ok(SentGuestMessage::parse(&self.invoke(&request).await?))
    }
}
