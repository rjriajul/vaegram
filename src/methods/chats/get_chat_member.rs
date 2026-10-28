use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::ChatMemberStatus;
use crate::errors::{Error, Result};
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
                match members.into_iter().find(|m| m.user.id == user_id) {
                    Some(member) => Ok(member),
                    None => self.left_member(user_id).await,
                }
            }
            ChatPeer::Channel(channel) => {
                let (user_id, participant) = self.resolve(&user_id.into()).await?;
                let request = functions::channels::GetParticipant { channel, participant };
                let found = match self.invoke(&request).await {
                    Ok(base::channels::ChannelParticipant::ChannelParticipant(found)) => *found,
                    Err(Error::Rpc(e)) if e.name == "USER_NOT_PARTICIPANT" => return self.left_member(user_id).await,
                    Err(e) => return Err(e),
                };
                let entities = Entities::new(self, found.users, found.chats);
                Ok(ChatMember::parse_channel(&found.participant, &entities))
            }
        }
    }

    // Telegram answers a non-member with no user object, so fetch it to fill the Left member. A chat that posts
    // as itself is no user and has nothing to fetch.
    async fn left_member(&self, user_id: i64) -> Result<ChatMember> {
        if user_id < 0 {
            return Ok(ChatMember::new(user_id, ChatMemberStatus::Left, &Entities::default()));
        }
        let id = vec![self.resolve_user(&PeerRef::Id(user_id)).await?];
        let users = self.invoke(&functions::users::GetUsers { id }).await?;
        let entities = Entities::new(self, users, Vec::new());
        Ok(ChatMember::new(user_id, ChatMemberStatus::Left, &entities))
    }
}

#[cfg(test)]
mod tests {
    use crate::enums::ChatMemberStatus;
    use crate::raw::functions::{channels, users};
    use crate::raw::{Identifiable, base, types};
    use crate::utils::get_channel_id;
    use crate::{Client, Error};

    const USER: i64 = 7;

    #[tokio::test]
    async fn non_member_is_left_with_the_user_filled_in() {
        let refused = crate::errors::RpcError::new(400, "USER_NOT_PARTICIPANT");
        // Vector<User> holding one user with only first_name (flags bit 1) set.
        let mut users = Vec::new();
        for word in [0x1cb5c415, 1, types::User::CONSTRUCTOR_ID, 1 << 1, 0] {
            users.extend_from_slice(&u32::to_le_bytes(word));
        }
        users.extend_from_slice(&USER.to_le_bytes());
        users.extend_from_slice(b"\x03Ann");
        let (client, mut sent) = Client::scripted(vec![Err(Error::Rpc(refused)), Ok(users)]);
        let channel = get_channel_id(9);
        {
            let mut peers = client.0.peers.lock().unwrap();
            let user = types::InputPeerUser { user_id: USER, access_hash: 1 };
            peers.insert(USER, base::InputPeer::InputPeerUser(Box::new(user)));
            let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
            peers.insert(channel, base::InputPeer::InputPeerChannel(Box::new(peer)));
        }
        let member = client.get_chat_member(channel, USER).await.unwrap();
        assert_eq!(member.status, ChatMemberStatus::Left);
        assert!(!member.is_member);
        assert_eq!((member.user.id, member.user.first_name.as_str()), (USER, "Ann"));

        let id = |body: &[u8]| u32::from_le_bytes(body[..4].try_into().unwrap());
        assert_eq!(id(&sent.try_recv().unwrap()), channels::GetParticipant::CONSTRUCTOR_ID);
        assert_eq!(id(&sent.try_recv().unwrap()), users::GetUsers::CONSTRUCTOR_ID);
    }

    #[tokio::test]
    async fn non_member_channel_is_left_without_a_user_lookup() {
        let refused = crate::errors::RpcError::new(400, "USER_NOT_PARTICIPANT");
        let (client, mut sent) = Client::scripted(vec![Err(Error::Rpc(refused))]);
        let (group, sender) = (get_channel_id(9), get_channel_id(11));
        {
            let mut peers = client.0.peers.lock().unwrap();
            for (id, channel_id) in [(group, 9), (sender, 11)] {
                let peer = types::InputPeerChannel { channel_id, access_hash: 1 };
                peers.insert(id, base::InputPeer::InputPeerChannel(Box::new(peer)));
            }
        }
        let member = client.get_chat_member(group, sender).await.unwrap();
        assert_eq!((member.status, member.is_member, member.user.id), (ChatMemberStatus::Left, false, sender));
        let id = |body: &[u8]| u32::from_le_bytes(body[..4].try_into().unwrap());
        assert_eq!(id(&sent.try_recv().unwrap()), channels::GetParticipant::CONSTRUCTOR_ID);
        assert!(sent.try_recv().is_err());
    }
}
