use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::{BusinessBotRights, BusinessRecipients, PeerRef};

impl Client {
    pub async fn update_connected_bot(
        &self,
        bot_id: impl Into<PeerRef>,
        rights: BusinessBotRights,
        recipients: &BusinessRecipients,
    ) -> Result<()> {
        if !(recipients.existing_chats
            || recipients.new_chats
            || recipients.contacts
            || recipients.non_contacts
            || !recipients.users.is_empty())
        {
            return Err(Error::InvalidArgument("a connected bot needs at least one kind of recipient"));
        }
        let (bot, recipients) = (self.resolve_user(&bot_id.into()).await?, recipients.write_bot(self).await?);
        let rights = Some(rights.write());
        let request = functions::account::UpdateConnectedBot { deleted: false, rights, bot, recipients };
        self.invoke(&request).await.map(drop)
    }
}
