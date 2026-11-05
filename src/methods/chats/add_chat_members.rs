use crate::Client;
use crate::errors::{Error, Result};
use crate::methods::advanced::ChatPeer;
use crate::raw::{base, functions, types};
use crate::types::{FailedToAddMember, PeerRef};

impl Client {
    /// Adds users to a basic group, supergroup or channel.
    ///
    /// Returns the users Telegram could not add, as [`FailedToAddMember`]. Fails with `Error::InvalidArgument` if
    /// `user_ids` is empty. `forward_limit` is how many recent messages new members of a basic group can see.
    /// Supergroups and channels ignore it. A basic group takes one request per user.
    pub async fn add_chat_members(
        &self,
        chat_id: impl Into<PeerRef>,
        user_ids: impl IntoIterator<Item = impl Into<PeerRef>>,
        forward_limit: i32,
    ) -> Result<Vec<FailedToAddMember>> {
        let user_ids: Vec<PeerRef> = user_ids.into_iter().map(Into::into).collect();
        if user_ids.is_empty() {
            return Err(Error::InvalidArgument("add_chat_members needs at least one user"));
        }
        let mut users = Vec::with_capacity(user_ids.len());
        for id in user_ids {
            users.push(self.resolve_user(&id).await?);
        }
        match self.resolve_chat(&chat_id.into()).await? {
            ChatPeer::Channel(channel) => {
                let base::messages::InvitedUsers::InvitedUsers(invited) =
                    self.invoke(&functions::channels::InviteToChannel { channel, users }).await?;
                Ok(failed_members(&invited))
            }
            ChatPeer::Group(chat_id) => {
                let mut missing = Vec::new();
                // Basic groups only take one user per call.
                for user_id in users {
                    let request = functions::messages::AddChatUser { chat_id, user_id, fwd_limit: forward_limit };
                    let base::messages::InvitedUsers::InvitedUsers(invited) = self.invoke(&request).await?;
                    missing.extend(failed_members(&invited));
                }
                Ok(missing)
            }
        }
    }
}

pub(super) fn failed_members(invited: &types::messages::InvitedUsers) -> Vec<FailedToAddMember> {
    invited.missing_invitees.iter().map(FailedToAddMember::parse).collect()
}
