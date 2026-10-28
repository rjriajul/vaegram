use crate::client::Me;
use crate::dispatcher::Entities;
use crate::enums::{ChatType, MediaKind, ServiceKind};
use crate::raw::{base, types};
use crate::types::Chat;
use crate::utils::get_peer_id;

pub(crate) const OUTGOING: u32 = 1 << 0;
pub(crate) const TEXT: u32 = 1 << 1;
pub(crate) const CAPTION: u32 = 1 << 2;
pub(crate) const REPLY: u32 = 1 << 3;
pub(crate) const FORWARDED: u32 = 1 << 4;
pub(crate) const VIA_BOT: u32 = 1 << 5;
pub(crate) const MENTIONED: u32 = 1 << 6;
pub(crate) const SCHEDULED: u32 = 1 << 7;
pub(crate) const PROTECTED: u32 = 1 << 8;
pub(crate) const TOPIC: u32 = 1 << 9;
pub(crate) const MEDIA_GROUP: u32 = 1 << 10;
pub(crate) const SERVICE: u32 = 1 << 11;
pub(crate) const SENDER_BOT: u32 = 1 << 12;
pub(crate) const SENDER_ME: u32 = 1 << 13;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PeerKinds {
    pub(crate) chat: Option<ChatType>,
    pub(crate) sender_bot: bool,
}

impl PeerKinds {
    pub(crate) fn message(raw: &base::Message, entities: &Entities) -> PeerKinds {
        let (peer, from, post) = match raw {
            base::Message::Message(m) => (&m.peer_id, m.from_id.as_ref(), m.post),
            base::Message::MessageService(m) => (&m.peer_id, m.from_id.as_ref(), m.post),
            base::Message::MessageEmpty(_) => return PeerKinds::default(),
        };
        let sender_bot = match from.unwrap_or(peer) {
            base::Peer::PeerUser(p) => matches!(entities.users.get(&p.user_id), Some(base::User::User(u)) if u.bot),
            _ => false,
        };
        PeerKinds { chat: Some(Chat::kind_of(peer, entities, post)), sender_bot }
    }

    pub(crate) fn callback(raw: &types::UpdateBotCallbackQuery, entities: &Entities) -> PeerKinds {
        let sender_bot = matches!(entities.users.get(&raw.user_id), Some(base::User::User(u)) if u.bot);
        PeerKinds { chat: Some(Chat::kind_of(&raw.peer, entities, false)), sender_bot }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Facts<'a> {
    pub(crate) flags: u32,
    pub(crate) media: Option<MediaKind>,
    pub(crate) service: Option<ServiceKind>,
    pub(crate) chat: Option<ChatType>,
    pub(crate) chat_id: i64,
    pub(crate) sender_id: i64,
    pub(crate) thread_id: i32,
    pub(crate) text: &'a str,
    pub(crate) data: &'a [u8],
    pub(crate) me: Option<&'a str>,
}

impl<'a> Facts<'a> {
    pub(crate) fn message(raw: &'a base::Message, kinds: PeerKinds, me: &'a Me) -> Facts<'a> {
        let m = match raw {
            base::Message::Message(m) => return Facts::plain(m, kinds, me),
            base::Message::MessageService(m) => m,
            base::Message::MessageEmpty(_) => return Facts::default(),
        };
        let mut facts = Facts::common(&m.peer_id, m.from_id.as_ref(), m.out, m.post, kinds, me);
        facts.flags |= SERVICE | flag(m.mentioned, MENTIONED);
        facts.service = Some(ServiceKind::of(&m.action, kinds.chat));
        facts.reply(m.reply_to.as_ref());
        facts
    }

    pub(crate) fn plain(m: &'a types::Message, kinds: PeerKinds, me: &'a Me) -> Facts<'a> {
        let mut facts = Facts::common(&m.peer_id, m.from_id.as_ref(), m.out, m.post, kinds, me);
        let media = m.media.as_ref().and_then(MediaKind::of);
        let text = match media {
            _ if m.message.is_empty() => 0,
            None | Some(MediaKind::WebPage) => TEXT,
            Some(_) => CAPTION,
        };
        facts.flags |= text
            | flag(m.fwd_from.is_some(), FORWARDED)
            | flag(m.via_bot_id.is_some(), VIA_BOT)
            | flag(m.mentioned, MENTIONED)
            | flag(m.from_scheduled, SCHEDULED)
            | flag(m.noforwards, PROTECTED)
            | flag(m.grouped_id.is_some(), MEDIA_GROUP);
        (facts.media, facts.text) = (media, &m.message);
        facts.reply(m.reply_to.as_ref());
        facts
    }

    pub(crate) fn callback(raw: &'a types::UpdateBotCallbackQuery, kinds: PeerKinds, me: &'a Me) -> Facts<'a> {
        let data = raw.data.as_deref().unwrap_or_default();
        Facts::query(get_peer_id(&raw.peer), raw.user_id, data, kinds, me)
    }

    pub(crate) fn query(chat_id: i64, sender_id: i64, data: &'a [u8], kinds: PeerKinds, me: &'a Me) -> Facts<'a> {
        let flags = flag(kinds.sender_bot, SENDER_BOT) | flag(sender_id == me.id, SENDER_ME);
        Facts { flags, chat: kinds.chat, chat_id, sender_id, data, me: me.username.as_deref(), ..Facts::default() }
    }

    pub(crate) fn inline(user_id: i64, query: &'a str, me: &'a Me) -> Facts<'a> {
        let flags = flag(!query.is_empty(), TEXT) | flag(user_id == me.id, SENDER_ME);
        Facts { flags, sender_id: user_id, text: query, me: me.username.as_deref(), ..Facts::default() }
    }

    /// Ephemeral messages are only filtered once parsed; `chat_id` is the one the parsed item settled on.
    pub(crate) fn ephemeral(m: &'a types::EphemeralMessage, chat_id: i64, kinds: PeerKinds, me: &'a Me) -> Facts<'a> {
        let mut facts = Facts::query(chat_id, get_peer_id(&m.from_id), &[], kinds, me);
        let media = m.media.as_ref().and_then(MediaKind::of);
        let text = match media {
            _ if m.message.is_empty() => 0,
            None | Some(MediaKind::WebPage) => TEXT,
            Some(_) => CAPTION,
        };
        facts.flags |= text | flag(m.out, OUTGOING | SENDER_ME) | flag(m.noforwards, PROTECTED);
        (facts.media, facts.text) = (media, &m.message);
        facts.reply(m.reply_to.as_ref());
        facts
    }

    fn common(
        peer: &base::Peer,
        from: Option<&base::Peer>,
        out: bool,
        post: bool,
        kinds: PeerKinds,
        me: &'a Me,
    ) -> Facts<'a> {
        let chat_id = get_peer_id(peer);
        // Same sender as `Message::parse`: a private chat without from_id is from its peer, or from us when outgoing.
        let sender_id = match (from, peer) {
            (Some(from), _) => get_peer_id(from),
            (None, base::Peer::PeerUser(_)) if out => me.id,
            (None, base::Peer::PeerUser(p)) => p.user_id,
            (None, _) if post => chat_id,
            (None, _) => 0,
        };
        let flags = flag(out, OUTGOING)
            | flag(kinds.sender_bot, SENDER_BOT)
            | flag(out || (sender_id != 0 && sender_id == me.id), SENDER_ME);
        Facts { flags, chat: kinds.chat, chat_id, sender_id, me: me.username.as_deref(), ..Facts::default() }
    }

    fn reply(&mut self, header: Option<&base::MessageReplyHeader>) {
        let Some(base::MessageReplyHeader::MessageReplyHeader(h)) = header else { return };
        if h.forum_topic {
            self.flags |= TOPIC;
            self.thread_id = h.reply_to_top_id.or(h.reply_to_msg_id).unwrap_or(0);
        }
        // In a topic, a message that only points at the topic's root isn't a reply.
        let reply = h.reply_to_msg_id.is_some() && !(h.forum_topic && h.reply_to_top_id.is_none());
        self.flags |= flag(reply, REPLY);
    }
}

fn flag(on: bool, bit: u32) -> u32 {
    if on { bit } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Client;
    use crate::filters::Inspect;
    use crate::raw::Serializable;
    use crate::types::{CallbackQuery, Message};
    use crate::utils::{get_channel_id, get_peer};

    fn user(id: i64, bot: bool) -> base::User {
        let mut buf = Vec::new();
        for word in [u32::from(bot) << 14, 0] {
            word.serialize(&mut buf);
        }
        id.serialize(&mut buf);
        // flags.14 is both `bot` and `bot_info_version`.
        if bot {
            0i32.serialize(&mut buf);
        }
        base::User::User(Box::new(types::User::deserialize_bare(&mut &buf[..]).unwrap()))
    }

    fn megagroup(id: i64) -> base::Chat {
        let mut buf = Vec::new();
        for word in [1u32 << 8, 0] {
            word.serialize(&mut buf);
        }
        id.serialize(&mut buf);
        "g".to_owned().serialize(&mut buf);
        base::ChatPhoto::ChatPhotoEmpty.serialize(&mut buf);
        0i32.serialize(&mut buf);
        base::Chat::Channel(Box::new(types::Channel::deserialize_bare(&mut &buf[..]).unwrap()))
    }

    fn entities() -> Entities {
        let users = [(7, user(7, true)), (8, user(8, false))].into();
        Entities { users, chats: [(get_channel_id(55), megagroup(55))].into() }
    }

    #[test]
    fn raw_and_parsed_messages_have_the_same_facts() {
        let (client, entities) = (Client::offline(), entities());
        let me = Me { id: 0, is_bot: true, username: Some("Bot".into()) };
        let group = get_peer(get_channel_id(55));
        let cases = [
            (group, Some(7), false, Some(ChatType::Supergroup), SENDER_BOT),
            (get_peer(7), None, false, Some(ChatType::Bot), SENDER_BOT),
            (get_peer(8), None, true, Some(ChatType::Private), OUTGOING | SENDER_ME),
        ];
        for (peer, from, out, chat, sender) in cases {
            let mut m = Message::bare(3, peer, 0);
            (m.out, m.mentioned, m.noforwards, m.grouped_id) = (out, true, true, Some(9));
            (m.from_id, m.message) = (from.map(get_peer), "/start@Bot hi".into());
            let raw = base::Message::Message(Box::new(m.clone()));
            let facts = Facts::message(&raw, PeerKinds::message(&raw, &entities), &me);
            let parsed = Message::parse(&client, Box::new(m), &entities);
            assert_eq!(facts, parsed.facts(&me));
            assert_eq!((facts.chat, facts.flags), (chat, TEXT | MENTIONED | PROTECTED | MEDIA_GROUP | sender));
            assert_eq!((facts.text, facts.me), ("/start@Bot hi", Some("Bot")));
        }
    }

    #[test]
    fn raw_and_parsed_callback_queries_have_the_same_facts() {
        let (client, entities) = (Client::offline(), entities());
        let me = Me { id: 8, ..Me::default() };
        for (user_id, peer) in [(7, get_peer(get_channel_id(55))), (8, get_peer(7))] {
            let raw = types::UpdateBotCallbackQuery {
                query_id: 1,
                user_id,
                peer,
                msg_id: 2,
                chat_instance: 3,
                data: Some(b"buy:1".to_vec()),
                game_short_name: None,
            };
            let facts = Facts::callback(&raw, PeerKinds::callback(&raw, &entities), &me);
            assert_eq!(facts, CallbackQuery::parse(&client, &raw, &entities).facts(&me));
            assert_eq!((facts.data, facts.sender_id), (&b"buy:1"[..], user_id));
        }
    }

    #[test]
    fn service_messages_have_a_kind() {
        let raw = types::MessageService {
            out: false,
            mentioned: false,
            media_unread: false,
            reactions_are_possible: false,
            silent: false,
            post: false,
            legacy: false,
            id: 1,
            from_id: None,
            peer_id: get_peer(-5),
            saved_peer_id: None,
            reply_to: None,
            date: 0,
            action: base::MessageAction::MessageActionPinMessage,
            reactions: None,
            ttl_period: None,
        };
        let raw = base::Message::MessageService(Box::new(raw));
        let (entities, me) = (Entities::default(), Me::default());
        let facts = Facts::message(&raw, PeerKinds::message(&raw, &entities), &me);
        let kind = (facts.flags, facts.service, facts.chat);
        assert_eq!(kind, (SERVICE, Some(ServiceKind::PinnedMessage), Some(ChatType::Group)));
    }
}
