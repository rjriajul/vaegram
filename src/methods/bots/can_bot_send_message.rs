use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn can_bot_send_message(&self, bot_id: impl Into<PeerRef>) -> Result<bool> {
        self.invoke(&functions::bots::CanSendMessage { bot: self.resolve_user(&bot_id.into()).await? }).await
    }
}
