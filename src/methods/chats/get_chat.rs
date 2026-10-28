use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{Chat, Community, PeerRef, User};
use crate::utils::get_peer;

impl Client {
    pub async fn get_chat(&self, chat_id: impl Into<PeerRef>) -> Result<Chat> {
        let chat_id = self.peer_id(chat_id.into()).await?;
        if chat_id > 0 {
            let id = self.resolve_user(&chat_id.into()).await?;
            let base::users::UserFull::UserFull(full) = self.invoke(&functions::users::GetFullUser { id }).await?;
            let types::users::UserFull { full_user: base::UserFull::UserFull(user), chats, users } = *full;
            let entities = Entities::new(self, users, chats);
            let mut chat = Chat::parse(&get_peer(chat_id), &entities, false);
            chat.bio = user.about.filter(|about| !about.is_empty());
            chat.community = Community::linked(chat_id, &entities);
            return Ok(chat);
        }
        let (full_chat, entities) = self.full_chat(self.resolve_chat(&chat_id.into()).await?).await?;
        let mut chat = Chat::parse(&get_peer(chat_id), &entities, false);
        let about = match full_chat {
            base::ChatFull::ChatFull(c) => {
                if let base::ChatParticipants::ChatParticipants(p) = &c.participants {
                    chat.members_count = Some(p.participants.len() as i32);
                }
                c.about
            }
            base::ChatFull::ChannelFull(c) => {
                chat.members_count = c.participants_count;
                chat.guard_bot = c.guard_bot_id.map(|id| User::parse(id, &entities));
                c.about
            }
            base::ChatFull::CommunityFull(c) => c.about,
        };
        chat.description = (!about.is_empty()).then_some(about);
        chat.community = Community::linked(chat_id, &entities);
        Ok(chat)
    }
}

#[cfg(test)]
mod tests {
    use crate::client::sent;
    use crate::raw::functions::{channels, contacts, messages, users};
    use crate::raw::{Identifiable, base, types};
    use crate::types::{ChatPermissions, ChatPrivileges};
    use crate::utils::get_channel_id;
    use crate::{Client, Error};

    const USER: i64 = 7;
    const GROUP: i64 = -5;

    #[tokio::test]
    async fn dispatches_on_chat_id_kind() {
        let (client, mut rx) = Client::recording();
        let channel = get_channel_id(9);
        {
            let mut peers = client.0.peers.lock().unwrap();
            let user = types::InputPeerUser { user_id: USER, access_hash: 1 };
            peers.insert(USER, base::InputPeer::InputPeerUser(Box::new(user)));
            let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
            peers.insert(channel, base::InputPeer::InputPeerChannel(Box::new(peer)));
        }
        let rx = &mut rx;
        let admin = ChatPrivileges { can_pin_messages: true, ..Default::default() };
        let muted = ChatPermissions::default();

        assert_eq!(sent(rx, client.get_chat(USER)).await, users::GetFullUser::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_chat(-5)).await, messages::GetFullChat::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_chat(channel)).await, channels::GetFullChannel::CONSTRUCTOR_ID);

        assert_eq!(sent(rx, client.get_chat_member(GROUP, USER)).await, messages::GetFullChat::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_chat_member(channel, USER)).await, channels::GetParticipant::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_chat_members(GROUP)).await, messages::GetFullChat::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_chat_members(channel)).await, channels::GetParticipants::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_chat_administrators(GROUP)).await, messages::GetFullChat::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_chat_administrators(channel)).await, channels::GetParticipants::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_chat_members_count(GROUP)).await, messages::GetChats::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_chat_members_count(channel)).await, channels::GetFullChannel::CONSTRUCTOR_ID);

        assert_eq!(sent(rx, client.ban_chat_member(GROUP, USER)).await, messages::DeleteChatUser::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.ban_chat_member(channel, USER)).await, channels::EditBanned::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.unban_chat_member(channel, USER)).await, channels::EditBanned::CONSTRUCTOR_ID);
        assert_eq!(
            sent(rx, client.restrict_chat_member(channel, USER, muted)).await,
            channels::EditBanned::CONSTRUCTOR_ID
        );
        assert_eq!(
            sent(rx, client.promote_chat_member(GROUP, USER, admin)).await,
            messages::EditChatAdmin::CONSTRUCTOR_ID
        );
        assert_eq!(
            sent(rx, client.promote_chat_member(channel, USER, admin)).await,
            channels::EditAdmin::CONSTRUCTOR_ID
        );
        assert_eq!(sent(rx, client.leave_chat(GROUP)).await, messages::DeleteChatUser::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.leave_chat(channel)).await, channels::LeaveChannel::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.set_chat_title(GROUP, "t")).await, messages::EditChatTitle::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.set_chat_title(channel, "t")).await, channels::EditTitle::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.set_chat_description(channel, "d")).await, messages::EditChatAbout::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.join_chat("t.me/+hash")).await, messages::ImportChatInvite::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.join_chat("@group")).await, contacts::ResolveUsername::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_users([USER, 8, 9])).await, users::GetUsers::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_users(["me"])).await, users::GetUsers::CONSTRUCTOR_ID);

        for result in [
            client.leave_chat(USER).await,
            client.set_chat_title(USER, "t").await,
            client.restrict_chat_member(GROUP, USER, muted).await,
            client.unban_chat_member(GROUP, USER).await,
        ] {
            assert!(matches!(result, Err(Error::InvalidArgument(_))));
        }
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn rejects_a_user_id_without_a_request() {
        let (client, mut rx) = Client::recording();
        let user = types::InputPeerUser { user_id: USER, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(USER, base::InputPeer::InputPeerUser(Box::new(user)));
        assert!(matches!(client.leave_chat(USER).await, Err(Error::InvalidArgument(_))));
        let muted = ChatPermissions::default();
        assert!(matches!(client.restrict_chat_member(GROUP, USER, muted).await, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn server_peer_id_invalid_stays_an_rpc_error() {
        let refused = crate::errors::RpcError::new(400, "PEER_ID_INVALID");
        let (client, _sent) = Client::scripted(vec![Err(Error::Rpc(refused))]);
        let result = client.leave_chat(GROUP).await;
        assert!(matches!(result, Err(Error::Rpc(e)) if e.name == "PEER_ID_INVALID"));
    }
}
