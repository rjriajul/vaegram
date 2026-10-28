use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InlineMessageId, InlineQueryResult, SentGuestMessage};

impl Client {
    pub async fn answer_guest_query(
        &self,
        guest_query_id: i64,
        result: impl Into<InlineQueryResult>,
    ) -> Result<SentGuestMessage> {
        let result = result.into().write(self).await?;
        let request = functions::messages::SetBotGuestChatResult { query_id: guest_query_id, result };
        Ok(SentGuestMessage { inline_message_id: InlineMessageId(self.invoke(&request).await?) })
    }
}
