use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::ChatMemberStatus;
use crate::errors::{Error, Result};
use crate::methods::advanced::ChatPeer;
use crate::raw::{base, functions};
use crate::types::{ChatMember, PeerRef};

impl Client {
    /// Returns one member of a chat.
    ///
    /// A user who is not in the chat comes back with the `Left` status instead of an error.
    ///
    /// ```no_run
    /// # async fn f(client: &vaegram::Client) -> vaegram::Result<()> {
    /// let member = client.get_chat_member(-1001234567890, 42).await?;
    /// # Ok(()) }
    /// ```
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
    // as itself is no user and has nothing to fetch, and a user this account never met can't be fetched.
    async fn left_member(&self, user_id: i64) -> Result<ChatMember> {
        let left = |entities: &Entities| ChatMember::new(user_id, ChatMemberStatus::Left, entities);
        if user_id < 0 {
            return Ok(left(&Entities::default()));
        }
        let id = match self.resolve_user(&PeerRef::Id(user_id)).await {
            Ok(user) => vec![user],
            Err(Error::PeerNotFound(_)) => return Ok(left(&Entities::default())),
            Err(e) => return Err(e),
        };
        let users = self.invoke(&functions::users::GetUsers { id }).await?;
        Ok(left(&Entities::new(self, users, Vec::new())))
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
        let refused = crate::errors::RpcError::parse(400, "USER_NOT_PARTICIPANT");
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
        let refused = crate::errors::RpcError::parse(400, "USER_NOT_PARTICIPANT");
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

    #[tokio::test]
    async fn a_user_never_met_is_left_in_a_basic_group() {
        use crate::raw::Serializable;

        // messages.chatFull of basic group 5 with its member list hidden, then no user for the lookup.
        let mut full = Vec::new();
        types::messages::ChatFull::CONSTRUCTOR_ID.serialize(&mut full);
        types::ChatFull::CONSTRUCTOR_ID.serialize(&mut full);
        0u32.serialize(&mut full);
        5i64.serialize(&mut full);
        String::new().serialize(&mut full);
        let participants = types::ChatParticipantsForbidden { chat_id: 5, self_participant: None };
        base::ChatParticipants::ChatParticipantsForbidden(Box::new(participants)).serialize(&mut full);
        types::PeerNotifySettings::CONSTRUCTOR_ID.serialize(&mut full);
        0u32.serialize(&mut full);
        Vec::<i64>::new().serialize(&mut full);
        Vec::<i64>::new().serialize(&mut full);
        let (client, _sent) = Client::scripted(vec![Ok(full), Ok(Vec::<i64>::new().to_bytes())]);
        let member = client.get_chat_member(-5, 777).await.unwrap();
        assert_eq!((member.status, member.is_member, member.user.id), (ChatMemberStatus::Left, false, 777));
    }
}
