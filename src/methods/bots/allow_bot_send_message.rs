use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn allow_bot_send_message(&self, bot_id: impl Into<PeerRef>) -> Result<()> {
        self.invoke(&functions::bots::AllowSendMessage { bot: self.resolve_user(&bot_id.into()).await? })
            .await
            .map(drop)
    }
}
