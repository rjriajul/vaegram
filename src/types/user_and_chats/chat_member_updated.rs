use crate::Client;
use crate::client::Me;
use crate::dispatcher::Entities;
use crate::enums::ChatMemberStatus;
use crate::raw::{base, types};
use crate::types::{Chat, ChatInviteLink, ChatMember, User};

#[derive(Debug, Clone)]
pub struct ChatMemberUpdated {
    pub chat: Chat,
    pub from_user: User,
    pub date: i32,
    pub old_chat_member: Option<ChatMember>,
    pub new_chat_member: Option<ChatMember>,
    pub invite_link: Option<ChatInviteLink>,
    pub via_join_request: bool,
    pub via_chat_folder_invite_link: bool,
    client: Client,
}

impl ChatMemberUpdated {
    pub(crate) fn parse_channel(client: &Client, raw: types::UpdateChannelParticipant, entities: &Entities) -> Self {
        let peer = base::Peer::PeerChannel(Box::new(types::PeerChannel { channel_id: raw.channel_id }));
        let self_request = matches!(
            &raw.new_participant,
            Some(base::ChannelParticipant::ChannelParticipantSelf(p)) if p.via_request
        );
        let (invite_link, public_request) = invite(raw.invite, entities);
        ChatMemberUpdated {
            chat: Chat::parse(&peer, entities, false),
            from_user: User::parse(raw.actor_id, entities),
            date: raw.date,
            old_chat_member: raw.prev_participant.as_ref().map(|p| ChatMember::parse_channel(p, entities)),
            new_chat_member: raw.new_participant.as_ref().map(|p| ChatMember::parse_channel(p, entities)),
            invite_link,
            via_join_request: public_request || self_request,
            via_chat_folder_invite_link: raw.via_chatlist,
            client: client.clone(),
        }
    }

    pub(crate) fn parse_chat(client: &Client, raw: types::UpdateChatParticipant, entities: &Entities) -> Self {
        let peer = base::Peer::PeerChat(Box::new(types::PeerChat { chat_id: raw.chat_id }));
        let (invite_link, via_join_request) = invite(raw.invite, entities);
        ChatMemberUpdated {
            chat: Chat::parse(&peer, entities, false),
            from_user: User::parse(raw.actor_id, entities),
            date: raw.date,
            old_chat_member: raw.prev_participant.as_ref().map(|p| ChatMember::parse_chat(p, entities)),
            new_chat_member: raw.new_participant.as_ref().map(|p| ChatMember::parse_chat(p, entities)),
            invite_link,
            via_join_request,
            via_chat_folder_invite_link: false,
            client: client.clone(),
        }
    }

    /// Blocking a bot reads as banning it from the private chat, and restarting it as letting it back in.
    pub(crate) fn parse_bot_stopped(
        client: &Client,
        me: &Me,
        raw: &types::UpdateBotStopped,
        entities: &Entities,
    ) -> Self {
        let bot = User {
            id: me.id,
            is_bot: me.is_bot,
            username: me.username.as_deref().map(Into::into),
            ..User::parse(me.id, entities)
        };
        let peer = base::Peer::PeerUser(Box::new(types::PeerUser { user_id: raw.user_id }));
        let (old, new) = if raw.stopped {
            (ChatMemberStatus::Member, ChatMemberStatus::Banned)
        } else {
            (ChatMemberStatus::Banned, ChatMemberStatus::Member)
        };
        let member = |status| ChatMember { user: bot.clone(), ..ChatMember::new(bot.id, status, entities) };
        ChatMemberUpdated {
            chat: Chat::parse(&peer, entities, false),
            from_user: User::parse(raw.user_id, entities),
            date: raw.date,
            old_chat_member: Some(member(old)),
            new_chat_member: Some(member(new)),
            invite_link: None,
            via_join_request: false,
            via_chat_folder_invite_link: false,
            client: client.clone(),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}

fn invite(invite: Option<base::ExportedChatInvite>, entities: &Entities) -> (Option<ChatInviteLink>, bool) {
    match invite {
        Some(base::ExportedChatInvite::ChatInvitePublicJoinRequests) => (None, true),
        Some(invite) => (ChatInviteLink::parse(invite, entities).ok(), false),
        None => (None, false),
    }
}

#[cfg(test)]
mod tests {
    use super::ChatMemberUpdated;
    use crate::Client;
    use crate::client::Me;
    use crate::dispatcher::Entities;
    use crate::enums::ChatMemberStatus;
    use crate::raw::{base, types};

    #[test]
    fn parses_channel_participant() {
        let new = types::ChannelParticipantSelf {
            via_request: true,
            user_id: 7,
            inviter_id: 1,
            date: 100,
            subscription_until_date: None,
            rank: None,
        };
        let raw = types::UpdateChannelParticipant {
            via_chatlist: true,
            channel_id: 42,
            date: 100,
            actor_id: 1,
            user_id: 7,
            prev_participant: None,
            new_participant: Some(base::ChannelParticipant::ChannelParticipantSelf(Box::new(new))),
            invite: Some(base::ExportedChatInvite::ChatInvitePublicJoinRequests),
            qts: 1,
        };
        let update = ChatMemberUpdated::parse_channel(&Client::offline(), raw, &Entities::default());
        assert_eq!((update.chat.id, update.from_user.id, update.date), (-1000000000042, 1, 100));
        assert_eq!(update.old_chat_member, None);
        let new = update.new_chat_member.unwrap();
        assert_eq!((new.user.id, new.status, new.joined_date), (7, ChatMemberStatus::Member, Some(100)));
        assert_eq!(update.invite_link, None);
        assert!(update.via_join_request && update.via_chat_folder_invite_link);
    }

    #[test]
    fn parses_chat_participant_with_link() {
        let invite = types::ChatInviteExported {
            revoked: false,
            permanent: true,
            request_needed: false,
            link: "https://t.me/+abc".into(),
            admin_id: 1,
            date: 50,
            start_date: None,
            expire_date: None,
            usage_limit: None,
            usage: None,
            requested: None,
            subscription_expired: None,
            title: None,
            subscription_pricing: None,
        };
        let creator = types::ChatParticipantCreator { user_id: 7, rank: None };
        let raw = types::UpdateChatParticipant {
            chat_id: 42,
            date: 100,
            actor_id: 1,
            user_id: 7,
            prev_participant: Some(base::ChatParticipant::ChatParticipantCreator(Box::new(creator))),
            new_participant: None,
            invite: Some(base::ExportedChatInvite::ChatInviteExported(Box::new(invite))),
            qts: 1,
        };
        let update = ChatMemberUpdated::parse_chat(&Client::offline(), raw, &Entities::default());
        assert_eq!(update.chat.id, -42);
        assert_eq!(update.old_chat_member.map(|m| m.status), Some(ChatMemberStatus::Owner));
        assert_eq!(update.new_chat_member, None);
        let link = update.invite_link.unwrap();
        assert_eq!((link.invite_link.as_str(), link.is_primary), ("https://t.me/+abc", true));
        assert!(!update.via_join_request && !update.via_chat_folder_invite_link);
    }

    #[test]
    fn bot_stopped_bans_the_bot() {
        let me = Me { id: 1000, is_bot: true, username: None };
        let raw = types::UpdateBotStopped { user_id: 7, date: 100, stopped: true, qts: 1 };
        let update = ChatMemberUpdated::parse_bot_stopped(&Client::offline(), &me, &raw, &Entities::default());
        assert_eq!((update.chat.id, update.from_user.id), (7, 7));
        let (old, new) = (update.old_chat_member.unwrap(), update.new_chat_member.unwrap());
        assert_eq!((old.user.id, old.status, new.status), (1000, ChatMemberStatus::Member, ChatMemberStatus::Banned));
        assert!(old.is_member && !new.is_member);
    }
}
