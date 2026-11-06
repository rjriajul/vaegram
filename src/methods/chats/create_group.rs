use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{CreatedGroup, PeerRef};

impl Client {
    /// Creates a basic group with the given users.
    ///
    /// Returns the new group and the users Telegram could not add, as a [`CreatedGroup`].
    pub async fn create_group(
        &self,
        title: impl Into<String>,
        user_ids: impl IntoIterator<Item = impl Into<PeerRef>>,
    ) -> Result<CreatedGroup> {
        let user_ids: Vec<PeerRef> = user_ids.into_iter().map(Into::into).collect();
        let mut users = Vec::with_capacity(user_ids.len());
        for id in user_ids {
            users.push(self.resolve_user(&id).await?);
        }
        let request = functions::messages::CreateChat { users, title: title.into(), ttl_period: None };
        let base::messages::InvitedUsers::InvitedUsers(invited) = self.invoke(&request).await?;
        let failed_members = super::add_chat_members::failed_members(&invited);
        Ok(CreatedGroup { chat: super::create_channel::created_chat(self, invited.updates)?, failed_members })
    }
}
