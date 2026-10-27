use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{ChatPermissions, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct RestrictChatMember<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    user_id: PeerRef,
    permissions: ChatPermissions,
    until_date: i32,
}

impl Client {
    /// Supergroups only. Pass `ChatPermissions::default()` to mute the user.
    pub fn restrict_chat_member(
        &self,
        chat_id: impl Into<PeerRef>,
        user_id: impl Into<PeerRef>,
        permissions: ChatPermissions,
    ) -> RestrictChatMember<'_> {
        RestrictChatMember {
            client: self,
            chat_id: chat_id.into(),
            user_id: user_id.into(),
            permissions,
            until_date: 0,
        }
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
            channel: client.resolve_channel(&self.chat_id).await?,
            participant: client.resolve_peer(self.user_id).await?,
            banned_rights: base::ChatBannedRights::ChatBannedRights(Box::new(self.permissions.write(self.until_date))),
        };
        client.invoke(&request).await?;
        Ok(())
    }
}

into_future!(RestrictChatMember, ());
