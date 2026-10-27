use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result, RpcError};
use crate::methods::advanced::ChatPeer;
use crate::raw::{base, functions};
use crate::types::{ChatMember, PeerRef};

impl Client {
    pub async fn get_chat_member(
        &self,
        chat_id: impl Into<PeerRef>,
        user_id: impl Into<PeerRef>,
    ) -> Result<ChatMember> {
        match self.resolve_chat(&chat_id.into()).await? {
            ChatPeer::Group(chat_id) => {
                let user_id = self.peer_id(user_id.into()).await?;
                let members = self.basic_group_members(chat_id).await?;
                members.into_iter().find(|m| m.user.id == user_id).ok_or_else(user_not_participant)
            }
            ChatPeer::Channel(channel) => {
                let participant = self.resolve_peer(user_id).await?;
                let request = functions::channels::GetParticipant { channel, participant };
                let base::channels::ChannelParticipant::ChannelParticipant(found) = self.invoke(&request).await?;
                let found = *found;
                let entities = Entities::new(self, found.users, found.chats);
                Ok(ChatMember::parse_channel(&found.participant, &entities))
            }
        }
    }
}

fn user_not_participant() -> Error {
    Error::Rpc(RpcError::new(400, "USER_NOT_PARTICIPANT"))
}
