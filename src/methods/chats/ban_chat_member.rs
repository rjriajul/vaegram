use crate::Client;
use crate::errors::Result;
use crate::methods::advanced::ChatPeer;
use crate::raw::{base, functions};
use crate::types::ChatPermissions;

pub struct BanChatMember<'a> {
    client: &'a Client,
    chat_id: i64,
    user_id: i64,
    until_date: i32,
}

impl Client {
    /// Basic groups only remove the user; a ban with `until_date` needs a supergroup or channel.
    pub fn ban_chat_member(&self, chat_id: i64, user_id: i64) -> BanChatMember<'_> {
        BanChatMember { client: self, chat_id, user_id, until_date: 0 }
    }
}

impl BanChatMember<'_> {
    /// Unix time when the ban ends; 0 or less than 30 seconds away bans forever.
    pub fn until_date(mut self, until_date: i32) -> Self {
        self.until_date = until_date;
        self
    }

    async fn send(self) -> Result<()> {
        let client = self.client;
        match client.resolve_chat(self.chat_id).await? {
            ChatPeer::Group(chat_id) => {
                let user_id = client.resolve_user(self.user_id).await?;
                let request = functions::messages::DeleteChatUser { revoke_history: false, chat_id, user_id };
                client.invoke(&request).await?;
            }
            ChatPeer::Channel(channel) => {
                let mut rights = ChatPermissions::default().write(self.until_date);
                rights.view_messages = true;
                let request = functions::channels::EditBanned {
                    channel,
                    participant: client.resolve_peer(self.user_id).await?,
                    banned_rights: base::ChatBannedRights::ChatBannedRights(Box::new(rights)),
                };
                client.invoke(&request).await?;
            }
        }
        Ok(())
    }
}

into_future!(BanChatMember, ());
