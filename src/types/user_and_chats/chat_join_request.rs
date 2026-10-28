use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::ChatJoinRequestResult;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::{Chat, User};

#[derive(Clone)]
pub struct ChatJoinRequest {
    pub chat: Chat,
    pub from_user: User,
    pub user_chat_id: i64,
    pub date: i32,
    pub bio: Option<String>,
    pub invite_link: Option<String>,
    pub query_id: Option<i64>,
    client: Client,
}

impl ChatJoinRequest {
    pub(crate) fn parse(
        client: &Client,
        raw: &types::UpdateBotChatInviteRequester,
        entities: &Entities,
    ) -> ChatJoinRequest {
        ChatJoinRequest {
            chat: Chat::parse(&raw.peer, entities, false),
            from_user: User::parse(raw.user_id, entities),
            user_chat_id: raw.user_id,
            date: raw.date,
            bio: (!raw.about.is_empty()).then(|| raw.about.clone()),
            invite_link: match &raw.invite {
                base::ExportedChatInvite::ChatInviteExported(i) => Some(i.link.clone()),
                base::ExportedChatInvite::ChatInvitePublicJoinRequests => None,
            },
            query_id: raw.query_id,
            client: client.clone(),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    /// A request with a `query_id` went to the chat's guard bot, which answers it with `bots.setJoinChatResults`.
    pub async fn approve(&self) -> Result<()> {
        match self.query_id {
            Some(id) => self.client.answer_chat_join_request_query(id, ChatJoinRequestResult::Approve).await.map(drop),
            None => self.client.approve_chat_join_request(self.chat.id, self.from_user.id).await,
        }
    }

    pub async fn decline(&self) -> Result<()> {
        match self.query_id {
            Some(id) => self.client.answer_chat_join_request_query(id, ChatJoinRequestResult::Decline).await.map(drop),
            None => self.client.decline_chat_join_request(self.chat.id, self.from_user.id).await,
        }
    }
}

impl fmt::Debug for ChatJoinRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChatJoinRequest")
            .field("chat", &self.chat)
            .field("from_user", &self.from_user)
            .field("user_chat_id", &self.user_chat_id)
            .field("date", &self.date)
            .field("bio", &self.bio)
            .field("invite_link", &self.invite_link)
            .field("query_id", &self.query_id)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::ChatJoinRequest;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::functions::{bots, messages};
    use crate::raw::{Identifiable, base, types};
    use crate::utils::get_channel_id;

    fn update(
        query_id: Option<i64>,
        about: &str,
        invite: base::ExportedChatInvite,
    ) -> types::UpdateBotChatInviteRequester {
        types::UpdateBotChatInviteRequester {
            peer: base::Peer::PeerChannel(Box::new(types::PeerChannel { channel_id: 42 })),
            date: 1_700_000_000,
            user_id: 7,
            about: about.into(),
            invite,
            qts: 1,
            query_id,
        }
    }

    #[test]
    fn parses_query_id_and_invite() {
        let client = Client::offline();
        let entities = Entities::default();
        let invite = types::ChatInviteExported {
            revoked: false,
            permanent: false,
            request_needed: true,
            link: "https://t.me/+abc".into(),
            admin_id: 1,
            date: 0,
            start_date: None,
            expire_date: None,
            usage_limit: None,
            usage: None,
            requested: None,
            subscription_expired: None,
            title: None,
            subscription_pricing: None,
        };
        let raw = update(Some(123), "hi", base::ExportedChatInvite::ChatInviteExported(Box::new(invite)));
        let request = ChatJoinRequest::parse(&client, &raw, &entities);
        assert_eq!(
            (request.query_id, request.chat.id, request.from_user.id, request.date),
            (Some(123), get_channel_id(42), 7, 1_700_000_000)
        );
        assert_eq!((request.bio.as_deref(), request.invite_link.as_deref()), (Some("hi"), Some("https://t.me/+abc")));

        let raw = update(None, "", base::ExportedChatInvite::ChatInvitePublicJoinRequests);
        let request = ChatJoinRequest::parse(&client, &raw, &entities);
        assert_eq!((request.query_id, request.bio, request.invite_link), (None, None, None));
    }

    #[tokio::test]
    async fn guard_bot_requests_answer_the_query() {
        let (client, mut rx) = Client::recording();
        let channel = types::InputPeerChannel { channel_id: 42, access_hash: 1 };
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(get_channel_id(42), base::InputPeer::InputPeerChannel(Box::new(channel)));
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        let (public, entities) = (base::ExportedChatInvite::ChatInvitePublicJoinRequests, Entities::default());
        let request = |query_id| ChatJoinRequest::parse(&client, &update(query_id, "", public.clone()), &entities);
        let _ = request(Some(1)).decline().await;
        let _ = request(None).approve().await;
        let mut sent = || u32::from_le_bytes(rx.try_recv().expect("no request sent")[..4].try_into().unwrap());
        let expected = (bots::SetJoinChatResults::CONSTRUCTOR_ID, messages::HideChatJoinRequest::CONSTRUCTOR_ID);
        assert_eq!((sent(), sent()), expected);
    }
}
