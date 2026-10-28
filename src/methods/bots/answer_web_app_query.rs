use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{InlineMessageId, InlineQueryResult};

impl Client {
    pub async fn answer_web_app_query(
        &self,
        web_app_query_id: impl Into<String>,
        result: impl Into<InlineQueryResult>,
    ) -> Result<Option<InlineMessageId>> {
        let request = functions::messages::SendWebViewResultMessage {
            bot_query_id: web_app_query_id.into(),
            result: result.into().write(self).await?,
        };
        let base::WebViewMessageSent::WebViewMessageSent(sent) = self.invoke(&request).await?;
        Ok(sent.msg_id.map(InlineMessageId))
    }
}
