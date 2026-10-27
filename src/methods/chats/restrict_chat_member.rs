use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::ChatPermissions;

pub struct RestrictChatMember<'a> {
    client: &'a Client,
    chat_id: i64,
    user_id: i64,
    permissions: ChatPermissions,
    until_date: i32,
}

impl Client {
    /// Supergroups only. Pass `ChatPermissions::default()` to mute the user.
    pub fn restrict_chat_member(
        &self,
        chat_id: i64,
        user_id: i64,
        permissions: ChatPermissions,
    ) -> RestrictChatMember<'_> {
        RestrictChatMember { client: self, chat_id, user_id, permissions, until_date: 0 }
    }
}

impl RestrictChatMember<'_> {
    pub fn until_date(mut self, until_date: i32) -> Self {
        self.until_date = until_date;
        self
    }

    async fn send(self) -> Result<()> {
        let client = self.client;
        let request = functions::channels::EditBanned {
            channel: client.resolve_channel(self.chat_id).await?,
            participant: client.resolve_peer(self.user_id).await?,
            banned_rights: base::ChatBannedRights::ChatBannedRights(Box::new(self.permissions.write(self.until_date))),
        };
        client.invoke(&request).await?;
        Ok(())
    }
}

into_future!(RestrictChatMember, ());
