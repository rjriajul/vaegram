use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Gives a bot permission to message you.
    ///
    /// Use [`can_bot_send_message`](Client::can_bot_send_message) to check whether it already can.
    pub async fn allow_bot_send_message(&self, bot_id: impl Into<PeerRef>) -> Result<()> {
        self.invoke(&functions::bots::AllowSendMessage { bot: self.resolve_user(&bot_id.into()).await? })
            .await
            .map(drop)
    }
}
