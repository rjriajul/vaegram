use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{BusinessRecipients, PeerRef};

impl Client {
    pub async fn remove_connected_bot(&self, bot_id: impl Into<PeerRef>) -> Result<()> {
        let bot = self.resolve_user(&bot_id.into()).await?;
        let recipients = BusinessRecipients::default().write_bot(self).await?;
        let request = functions::account::UpdateConnectedBot { deleted: true, rights: None, bot, recipients };
        self.invoke(&request).await.map(drop)
    }
}
