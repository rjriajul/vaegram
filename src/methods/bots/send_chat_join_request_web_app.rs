use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};

impl Client {
    pub async fn send_chat_join_request_web_app(&self, query_id: i64, web_app_url: impl Into<String>) -> Result<bool> {
        let web_view = types::JoinChatBotResultWebView { url: web_app_url.into() };
        let result = base::JoinChatBotResult::JoinChatBotResultWebView(Box::new(web_view));
        self.invoke(&functions::bots::SetJoinChatResults { query_id, result }).await
    }
}
