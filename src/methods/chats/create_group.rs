use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{Chat, PeerRef};

impl Client {
    pub async fn create_group(
        &self,
        title: impl Into<String>,
        user_ids: impl IntoIterator<Item = impl Into<PeerRef>>,
    ) -> Result<(Chat, Vec<i64>)> {
        let user_ids: Vec<PeerRef> = user_ids.into_iter().map(Into::into).collect();
        let mut users = Vec::with_capacity(user_ids.len());
        for id in user_ids {
            users.push(self.resolve_user(&id).await?);
        }
        let request = functions::messages::CreateChat { users, title: title.into(), ttl_period: None };
        let base::messages::InvitedUsers::InvitedUsers(invited) = self.invoke(&request).await?;
        let missing = super::add_chat_members::missing_ids(&invited);
        Ok((super::create_channel::created_chat(self, invited.updates)?, missing))
    }
}
