use crate::client::Me;
use crate::dispatcher::Entities;
use crate::enums::{ChatType, GiftPurchaseOfferState, MediaKind, ServiceKind};
use crate::raw::{base, types};
use crate::types::{Chat, Message, ServiceAction};
use crate::utils::get_peer_id;

pub(crate) const OUTGOING: u32 = 1 << 0;
pub(crate) const TEXT: u32 = 1 << 1;
pub(crate) const CAPTION: u32 = 1 << 2;
pub(crate) const REPLY: u32 = 1 << 3;
pub(crate) const FORWARDED: u32 = 1 << 4;
pub(crate) const VIA_BOT: u32 = 1 << 5;
pub(crate) const MENTIONED: u32 = 1 << 6;
pub(crate) const FROM_SCHEDULED: u32 = 1 << 7;
pub(crate) const PROTECTED: u32 = 1 << 8;
pub(crate) const TOPIC: u32 = 1 << 9;
pub(crate) const MEDIA_GROUP: u32 = 1 << 10;
pub(crate) const SERVICE: u32 = 1 << 11;
pub(crate) const SENDER_BOT: u32 = 1 << 12;
pub(crate) const SENDER_ME: u32 = 1 << 13;
pub(crate) const SENDER_CHAT: u32 = 1 << 14;
pub(crate) const QUOTE: u32 = 1 << 15;
pub(crate) const INLINE_KEYBOARD: u32 = 1 << 16;
pub(crate) const REPLY_KEYBOARD: u32 = 1 << 17;
pub(crate) const MEDIA_SPOILER: u32 = 1 << 18;
pub(crate) const LINKED_CHANNEL: u32 = 1 << 19;
pub(crate) const LIVE_LOCATION: u32 = 1 << 20;
pub(crate) const SELF_DESTRUCTION: u32 = 1 << 21;
pub(crate) const PAID_MESSAGE: u32 = 1 << 22;
pub(crate) const BUSINESS: u32 = 1 << 23;
pub(crate) const FORUM: u32 = 1 << 24;
pub(crate) const ADMIN: u32 = 1 << 25;
// A message that waits for its date. Only parsed messages carry it: the updates handlers get are never scheduled.
pub(crate) const SCHEDULED: u32 = 1 << 26;
pub(crate) const OFFER_ACCEPTED: u32 = 1 << 27;
pub(crate) const OFFER_REJECTED: u32 = 1 << 28;
pub(crate) const REACTED: u32 = 1 << 29;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PeerKinds {
    pub(crate) chat: Option<ChatType>,
    pub(crate) sender_bot: bool,
    pub(crate) forum: bool,
    pub(crate) admin: bool,
}

impl PeerKinds {
    /// What a parsed update knows about its chat; `chat_traits` reads the same from raw entities.
    pub(crate) fn of(chat: Option<&Chat>, sender_bot: bool) -> PeerKinds {
        let (forum, admin) = chat.map_or((false, false), |c| (c.is_forum, c.is_admin));
        PeerKinds { chat: chat.map(|c| c.kind), sender_bot, forum, admin }
    }

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
        let (forum, admin) = chat_traits(peer, entities);
        PeerKinds { chat: Some(Chat::kind_of(peer, entities, post)), sender_bot, forum, admin }
    }

    pub(crate) fn callback(raw: &types::UpdateBotCallbackQuery, entities: &Entities) -> PeerKinds {
        let sender_bot = matches!(entities.users.get(&raw.user_id), Some(base::User::User(u)) if u.bot);
        let (forum, admin) = chat_traits(&raw.peer, entities);
        PeerKinds { chat: Some(Chat::kind_of(&raw.peer, entities, false)), sender_bot, forum, admin }
    }

    fn flags(self) -> u32 {
        flag(self.sender_bot, SENDER_BOT) | flag(self.forum, FORUM) | flag(self.admin, ADMIN)
    }
}

/// `(forum, admin)` from the raw entities, the same rules as `Chat::parse`.
fn chat_traits(peer: &base::Peer, entities: &Entities) -> (bool, bool) {
    match (peer, entities.chats.get(&get_peer_id(peer))) {
        (base::Peer::PeerChannel(_), Some(base::Chat::Channel(c))) => (c.forum, c.admin_rights.is_some()),
        (base::Peer::PeerChat(_), Some(base::Chat::Chat(c))) => (false, c.admin_rights.is_some()),
        _ => (false, false),
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
        facts.flags |= SERVICE | flag(m.mentioned, MENTIONED) | reacted(m.reactions.as_ref());
        facts.service = Some(ServiceKind::of(&m.action, kinds.chat));
        facts.reply(m.reply_to.as_ref());
        match &m.action {
            // A pin's header points at the pinned message, which the pin doesn't reply to.
            base::MessageAction::MessageActionPinMessage => facts.flags &= !REPLY,
            // The message that creates a topic is its root, so the topic id is the message's own id.
            base::MessageAction::MessageActionTopicCreate(_) => {
                facts.flags |= TOPIC;
                facts.thread_id = m.id;
            }
            base::MessageAction::MessageActionStarGiftPurchaseOffer(offer) => {
                // `UpgradedGiftPurchaseOffer::parse` reads `accepted` first, so both flags together mean accepted.
                facts.flags |=
                    flag(offer.accepted, OFFER_ACCEPTED) | flag(offer.declined && !offer.accepted, OFFER_REJECTED);
            }
            _ => {}
        }
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
            | flag(m.from_scheduled, FROM_SCHEDULED)
            | flag(m.noforwards, PROTECTED)
            | flag(m.grouped_id.is_some(), MEDIA_GROUP)
            | flag(m.paid_message_stars.is_some_and(|stars| stars > 0), PAID_MESSAGE)
            | flag(linked_channel(m.fwd_from.as_ref(), facts.sender_id), LINKED_CHANNEL)
            | media_traits(m.media.as_ref())
            | keyboard(m.reply_markup.as_ref())
            | reacted(m.reactions.as_ref());
        (facts.media, facts.text) = (media, &m.message);
        facts.reply(m.reply_to.as_ref());
        facts
    }

    /// A service message's `raw` is a bare message, so its kind and topic come from the parsed fields. A business
    /// message's connection isn't in `raw` either; the raw path adds it through `via_business`.
    pub(crate) fn parsed(m: &'a Message, me: &'a Me) -> Facts<'a> {
        let sender_bot = m.from_user.as_ref().is_some_and(|u| u.is_bot);
        let mut facts = Facts::plain(&m.raw, PeerKinds::of(Some(&m.chat), sender_bot), me)
            .via_business(m.business_connection_id.is_some());
        if let Some(service) = &m.service {
            facts.flags |= SERVICE;
            facts.service = Some(service.kind());
            if let ServiceAction::UpgradedGiftPurchaseOffer(offer) = service {
                facts.flags |= flag(offer.state == GiftPurchaseOfferState::Accepted, OFFER_ACCEPTED)
                    | flag(offer.state == GiftPurchaseOfferState::Rejected, OFFER_REJECTED);
            }
        }
        facts.flags |= flag(m.scheduled, SCHEDULED);
        if m.is_topic_message {
            facts.flags |= TOPIC;
            facts.thread_id = m.message_thread_id.unwrap_or(0);
        }
        facts
    }

    pub(crate) fn callback(raw: &'a types::UpdateBotCallbackQuery, kinds: PeerKinds, me: &'a Me) -> Facts<'a> {
        let data = raw.data.as_deref().unwrap_or_default();
        Facts::query(get_peer_id(&raw.peer), raw.user_id, data, kinds, me)
    }

    pub(crate) fn query(chat_id: i64, sender_id: i64, data: &'a [u8], kinds: PeerKinds, me: &'a Me) -> Facts<'a> {
        let flags = kinds.flags() | flag(sender_id == me.id, SENDER_ME) | flag(sender_id < 0, SENDER_CHAT);
        Facts { flags, chat: kinds.chat, chat_id, sender_id, data, me: me.username.as_deref(), ..Facts::default() }
    }

    pub(crate) fn via_business(mut self, on: bool) -> Facts<'a> {
        self.flags |= flag(on, BUSINESS);
        self
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
        // `kinds` looked at the peer; an outgoing private message without from_id is sent by us.
        let own = from.is_none() && out && matches!(peer, base::Peer::PeerUser(_));
        let kinds = if own { PeerKinds { sender_bot: me.is_bot, ..kinds } } else { kinds };
        let flags = flag(out, OUTGOING)
            | kinds.flags()
            | flag(out || (sender_id != 0 && sender_id == me.id), SENDER_ME)
            | flag(sender_id < 0, SENDER_CHAT);
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
        self.flags |= flag(reply, REPLY) | flag(h.quote_text.is_some(), QUOTE);
    }
}

fn flag(on: bool, bit: u32) -> u32 {
    if on { bit } else { 0 }
}

// The same rules as `Message::has_media_spoiler`, `Message::ttl_seconds` and a live `Location`, read off the raw media.
fn media_traits(media: Option<&base::MessageMedia>) -> u32 {
    let timed = |ttl: Option<i32>| flag(ttl.is_some_and(|ttl| ttl > 0), SELF_DESTRUCTION);
    match media {
        Some(base::MessageMedia::MessageMediaPhoto(m)) => flag(m.spoiler, MEDIA_SPOILER) | timed(m.ttl_seconds),
        Some(base::MessageMedia::MessageMediaDocument(m)) => flag(m.spoiler, MEDIA_SPOILER) | timed(m.ttl_seconds),
        Some(base::MessageMedia::MessageMediaGeoLive(m)) => flag(m.period > 0, LIVE_LOCATION),
        _ => 0,
    }
}

fn keyboard(markup: Option<&base::ReplyMarkup>) -> u32 {
    match markup {
        Some(base::ReplyMarkup::ReplyInlineMarkup(_)) => INLINE_KEYBOARD,
        Some(base::ReplyMarkup::ReplyKeyboardMarkup(_)) => REPLY_KEYBOARD,
        _ => 0,
    }
}

/// At least one reaction is counted in the reaction block.
fn reacted(reactions: Option<&base::MessageReactions>) -> u32 {
    let Some(base::MessageReactions::MessageReactions(r)) = reactions else { return 0 };
    flag(r.results.iter().any(|base::ReactionCount::ReactionCount(c)| c.count > 0), REACTED)
}

/// An automatic forward from a linked channel: a channel post whose origin is the chat that sent it.
fn linked_channel(header: Option<&base::MessageFwdHeader>, sender_id: i64) -> bool {
    let Some(base::MessageFwdHeader::MessageFwdHeader(h)) = header else { return false };
    match &h.from_id {
        Some(base::Peer::PeerUser(_)) | None => false,
        Some(origin) => h.channel_post.is_some() && get_peer_id(origin) == sender_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Client;
    use crate::filters::Inspect;
    use crate::raw::Serializable;
    use crate::types::messages_and_media::message::tests::{header, service};
    use crate::types::{CallbackQuery, Message, MessageReactionUpdated};
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

    // flags.5 is broadcast, flags.8 megagroup and flags.30 forum.
    fn channel(id: i64, flags: u32) -> types::Channel {
        let mut buf = Vec::new();
        for word in [flags, 0] {
            word.serialize(&mut buf);
        }
        id.serialize(&mut buf);
        "g".to_owned().serialize(&mut buf);
        base::ChatPhoto::ChatPhotoEmpty.serialize(&mut buf);
        0i32.serialize(&mut buf);
        types::Channel::deserialize_bare(&mut &buf[..]).unwrap()
    }

    fn megagroup(id: i64) -> base::Chat {
        base::Chat::Channel(Box::new(channel(id, 1 << 8)))
    }

    fn entities() -> Entities {
        let users = [(7, user(7, true)), (8, user(8, false))].into();
        Entities { users, chats: [(get_channel_id(55), megagroup(55))].into() }
    }

    #[test]
    fn admin_comes_from_admin_rights_on_both_paths() {
        let (client, me) = (Client::offline(), Me::default());
        // flags.4 is ban_users.
        let rights = || {
            let rights = types::ChatAdminRights::deserialize_bare(&mut &16i32.to_le_bytes()[..]).unwrap();
            Some(base::ChatAdminRights::ChatAdminRights(rights.into()))
        };
        let group = |admin_rights| {
            base::Chat::Chat(Box::new(types::Chat {
                creator: false,
                left: false,
                deactivated: false,
                call_active: false,
                call_not_empty: false,
                noforwards: false,
                id: 3,
                title: "g".into(),
                photo: base::ChatPhoto::ChatPhotoEmpty,
                participants_count: 2,
                date: 0,
                version: 1,
                migrated_to: None,
                admin_rights,
                default_banned_rights: None,
            }))
        };
        let broadcast =
            |admin_rights| base::Chat::Channel(Box::new(types::Channel { admin_rights, ..channel(5, 1 << 5) }));
        let cases = [
            (-3, group(rights()), true),
            (-3, group(None), false),
            (get_channel_id(5), broadcast(rights()), true),
            (get_channel_id(5), broadcast(None), false),
        ];
        for (i, (id, chat, admin)) in cases.into_iter().enumerate() {
            let entities = Entities { chats: [(id, chat)].into(), ..entities() };
            let mut m = Message::bare(3, get_peer(id), 0);
            m.from_id = Some(get_peer(8));
            let raw = base::Message::Message(Box::new(m.clone()));
            let facts = Facts::message(&raw, PeerKinds::message(&raw, &entities), &me);
            assert_eq!(facts, Message::parse(&client, Box::new(m), &entities).facts(&me), "case {i}");
            assert_eq!(facts.flags & ADMIN != 0, admin, "case {i}");

            let raw = types::UpdateBotCallbackQuery {
                query_id: 1,
                user_id: 8,
                peer: get_peer(id),
                msg_id: 2,
                chat_instance: 3,
                data: None,
                game_short_name: None,
            };
            let facts = Facts::callback(&raw, PeerKinds::callback(&raw, &entities), &me);
            assert_eq!(facts, CallbackQuery::parse(&client, &raw, &entities).facts(&me), "case {i}");
            assert_eq!(facts.flags & ADMIN != 0, admin, "case {i}");
        }
    }

    #[test]
    fn forum_comes_from_the_chat_on_both_paths() {
        let (client, me) = (Client::offline(), Me::default());
        for (flags, forum) in [((1 << 8) | (1 << 30), true), (1 << 8, false)] {
            let chat = base::Chat::Channel(Box::new(channel(56, flags)));
            let entities = Entities { chats: [(get_channel_id(56), chat)].into(), ..entities() };
            let mut m = Message::bare(3, get_peer(get_channel_id(56)), 0);
            m.from_id = Some(get_peer(8));
            let raw = base::Message::Message(Box::new(m.clone()));
            let facts = Facts::message(&raw, PeerKinds::message(&raw, &entities), &me);
            assert_eq!(facts, Message::parse(&client, Box::new(m), &entities).facts(&me));
            assert_eq!(facts.flags & FORUM != 0, forum);

            let raw = types::UpdateBotCallbackQuery {
                query_id: 1,
                user_id: 8,
                peer: get_peer(get_channel_id(56)),
                msg_id: 2,
                chat_instance: 3,
                data: None,
                game_short_name: None,
            };
            let facts = Facts::callback(&raw, PeerKinds::callback(&raw, &entities), &me);
            assert_eq!(facts, CallbackQuery::parse(&client, &raw, &entities).facts(&me));
            assert_eq!(facts.flags & FORUM != 0, forum);
        }
    }

    #[test]
    fn raw_and_parsed_messages_have_the_same_facts() {
        let (client, mut entities) = (Client::offline(), entities());
        // We are a bot too, so a message we sent is from a bot whatever the peer is.
        entities.users.insert(9, user(9, true));
        let me = Me { id: 9, is_bot: true, username: Some("Bot".into()), ..Me::default() };
        let group = get_peer(get_channel_id(55));
        let cases = [
            (group, Some(7), false, Some(ChatType::Supergroup), SENDER_BOT),
            (get_peer(7), None, false, Some(ChatType::Bot), SENDER_BOT),
            (get_peer(8), None, true, Some(ChatType::Private), OUTGOING | SENDER_ME | SENDER_BOT),
            (get_peer(7), None, true, Some(ChatType::Bot), OUTGOING | SENDER_ME | SENDER_BOT),
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
    fn our_message_to_a_bot_is_not_from_a_bot_when_we_are_not_one() {
        let (client, mut entities) = (Client::offline(), entities());
        entities.users.insert(9, user(9, false));
        let me = Me { id: 9, is_bot: false, ..Me::default() };
        let mut m = Message::bare(3, get_peer(7), 0);
        m.out = true;
        let raw = base::Message::Message(Box::new(m.clone()));
        let facts = Facts::message(&raw, PeerKinds::message(&raw, &entities), &me);
        assert_eq!(facts.flags & SENDER_BOT, 0);
        assert_eq!(facts, Message::parse(&client, Box::new(m), &entities).facts(&me));
    }

    #[test]
    fn reacted_needs_a_counted_reaction_on_both_paths() {
        let (client, entities, me) = (Client::offline(), entities(), Me::default());
        let block = |counts: &[i32]| {
            let results = counts
                .iter()
                .map(|&count| {
                    let reaction =
                        base::Reaction::ReactionEmoji(Box::new(types::ReactionEmoji { emoticon: "👍".into() }));
                    base::ReactionCount::ReactionCount(Box::new(types::ReactionCount {
                        chosen_order: None,
                        reaction,
                        count,
                    }))
                })
                .collect();
            let raw = types::MessageReactions {
                min: false,
                can_see_list: false,
                reactions_as_tags: false,
                results,
                recent_reactions: None,
                top_reactors: None,
            };
            Some(base::MessageReactions::MessageReactions(Box::new(raw)))
        };
        let filter: crate::filters::Filter = crate::filters::reacted();
        for (reactions, reacted) in [(None, false), (block(&[]), false), (block(&[0]), false), (block(&[0, 2]), true)] {
            let m = edited(|m| m.reactions = reactions.clone());
            let raw = base::Message::Message(Box::new(m.clone()));
            let facts = Facts::message(&raw, PeerKinds::message(&raw, &entities), &me);
            let parsed = Message::parse(&client, Box::new(m), &entities);
            assert_eq!(facts, parsed.facts(&me));
            assert_eq!((facts.flags & REACTED != 0, filter.check(&parsed)), (reacted, reacted), "{reactions:?}");

            let photo =
                types::MessageService { reactions, ..service(base::MessageAction::MessageActionChatDeletePhoto) };
            let raw = base::Message::MessageService(Box::new(photo));
            let facts = Facts::message(&raw, PeerKinds::message(&raw, &entities), &me);
            let parsed = Message::from_raw(&client, raw.clone(), &entities).unwrap();
            assert_eq!(facts, parsed.facts(&me));
            assert_eq!((facts.flags & REACTED != 0, filter.check(&parsed)), (reacted, reacted));
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
            from_id: None,
            peer_id: get_peer(-5),
            ..service(base::MessageAction::MessageActionPinMessage)
        };
        let raw = base::Message::MessageService(Box::new(raw));
        let (entities, me) = (Entities::default(), Me::default());
        let facts = Facts::message(&raw, PeerKinds::message(&raw, &entities), &me);
        let kind = (facts.flags, facts.service, facts.chat);
        assert_eq!(kind, (SERVICE, Some(ServiceKind::PinnedMessage), Some(ChatType::Group)));
    }

    #[test]
    fn raw_and_parsed_service_messages_have_the_same_facts() {
        let (client, entities) = (Client::offline(), entities());
        let me = Me { id: 0, is_bot: true, username: Some("Bot".into()), ..Me::default() };
        let title = types::MessageActionChatEditTitle { title: "t".into() };
        let title = service(base::MessageAction::MessageActionChatEditTitle(Box::new(title)));
        let pin = types::MessageReplyHeader {
            forum_topic: true,
            reply_to_msg_id: Some(12),
            reply_to_top_id: Some(4),
            ..header()
        };
        let pin = Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(pin)));
        let pin = types::MessageService { reply_to: pin, ..service(base::MessageAction::MessageActionPinMessage) };
        let create = types::MessageActionTopicCreate {
            title_missing: false,
            title: "t".into(),
            icon_color: 0,
            icon_emoji_id: None,
        };
        let create = service(base::MessageAction::MessageActionTopicCreate(Box::new(create)));
        let cases = [
            (types::MessageService { mentioned: true, ..title }, MENTIONED, ServiceKind::NewChatTitle, 0),
            (pin, TOPIC, ServiceKind::PinnedMessage, 4),
            (create, TOPIC, ServiceKind::ForumTopicCreated, 9),
        ];
        for (raw, flags, kind, thread_id) in cases {
            let raw = base::Message::MessageService(Box::new(raw));
            let facts = Facts::message(&raw, PeerKinds::message(&raw, &entities), &me);
            let parsed = Message::from_raw(&client, raw.clone(), &entities).unwrap();
            assert_eq!(facts, parsed.facts(&me));
            let expected = (SERVICE | SENDER_BOT | flags, Some(kind), thread_id);
            assert_eq!((facts.flags, facts.service, facts.thread_id), expected);
        }
    }

    const NEW: u32 = SENDER_CHAT
        | QUOTE
        | INLINE_KEYBOARD
        | REPLY_KEYBOARD
        | MEDIA_SPOILER
        | LINKED_CHANNEL
        | LIVE_LOCATION
        | SELF_DESTRUCTION
        | PAID_MESSAGE;

    fn edited(edit: impl FnOnce(&mut types::Message)) -> types::Message {
        let mut m = Message::bare(3, get_peer(8), 0);
        edit(&mut m);
        m
    }

    fn forward(from: i64) -> base::MessageFwdHeader {
        base::MessageFwdHeader::MessageFwdHeader(Box::new(types::MessageFwdHeader {
            imported: false,
            saved_out: false,
            from_id: Some(get_peer(from)),
            from_name: None,
            date: 1,
            channel_post: Some(5),
            post_author: None,
            saved_from_peer: None,
            saved_from_msg_id: None,
            saved_from_id: None,
            saved_from_name: None,
            saved_date: None,
            psa_type: None,
        }))
    }

    #[test]
    fn new_message_bits_match_raw_and_parsed() {
        let (client, entities, me) = (Client::offline(), entities(), Me::default());
        let inline = base::ReplyMarkup::ReplyInlineMarkup(Box::new(types::ReplyInlineMarkup {
            force_reply: false,
            rows: vec![],
        }));
        let keyboard = base::ReplyMarkup::ReplyKeyboardMarkup(Box::new(types::ReplyKeyboardMarkup {
            resize: false,
            single_use: false,
            selective: false,
            persistent: false,
            force_reply: false,
            rows: vec![],
            placeholder: None,
        }));
        let force_reply = base::ReplyMarkup::ReplyKeyboardForceReply(Box::new(types::ReplyKeyboardForceReply {
            single_use: false,
            selective: false,
            placeholder: None,
        }));
        let hide = base::ReplyMarkup::ReplyKeyboardHide(types::ReplyKeyboardHide { selective: false });
        let photo = |spoiler, ttl_seconds| {
            let photo = types::MessageMediaPhoto { spoiler, live_photo: false, photo: None, ttl_seconds, video: None };
            Some(base::MessageMedia::MessageMediaPhoto(Box::new(photo)))
        };
        let document = types::MessageMediaDocument {
            nopremium: false,
            spoiler: false,
            video: false,
            round: false,
            voice: false,
            document: None,
            alt_documents: None,
            video_cover: None,
            video_timestamp: None,
            ttl_seconds: Some(10),
        };
        let live = |period| {
            let live = types::MessageMediaGeoLive {
                geo: base::GeoPoint::GeoPointEmpty,
                heading: None,
                period,
                proximity_notification_radius: None,
            };
            Some(base::MessageMedia::MessageMediaGeoLive(Box::new(live)))
        };
        let quoted = types::MessageReplyHeader {
            reply_to_msg_id: Some(2),
            quote: true,
            quote_text: Some("q".into()),
            ..header()
        };
        let manual_flag_only = types::MessageReplyHeader { reply_to_msg_id: Some(2), quote: true, ..header() };
        let group = get_peer(get_channel_id(55));
        let auto_forward = |origin| {
            edited(|m| {
                (m.peer_id, m.from_id) = (group.clone(), Some(get_peer(get_channel_id(77))));
                m.fwd_from = Some(forward(get_channel_id(origin)));
            })
        };
        let cases = [
            (edited(|m| m.reply_markup = Some(inline)), INLINE_KEYBOARD),
            (edited(|m| m.reply_markup = Some(keyboard)), REPLY_KEYBOARD),
            (edited(|m| m.reply_markup = Some(force_reply)), 0),
            (edited(|m| m.reply_markup = Some(hide)), 0),
            (edited(|m| m.media = photo(true, None)), MEDIA_SPOILER),
            (edited(|m| m.media = photo(false, Some(0))), 0),
            (
                edited(|m| m.media = Some(base::MessageMedia::MessageMediaDocument(Box::new(document)))),
                SELF_DESTRUCTION,
            ),
            (edited(|m| m.media = live(0)), 0),
            (edited(|m| m.media = live(900)), LIVE_LOCATION),
            (edited(|m| m.paid_message_stars = Some(0)), 0),
            (edited(|m| m.paid_message_stars = Some(5)), PAID_MESSAGE),
            (edited(|m| m.reply_to = Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(quoted)))), QUOTE),
            (
                edited(|m| m.reply_to = Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(manual_flag_only)))),
                0,
            ),
            (auto_forward(77), LINKED_CHANNEL | SENDER_CHAT),
            (auto_forward(78), SENDER_CHAT),
        ];
        for (i, (m, bits)) in cases.into_iter().enumerate() {
            let raw = base::Message::Message(Box::new(m.clone()));
            let facts = Facts::message(&raw, PeerKinds::message(&raw, &entities), &me);
            assert_eq!(facts, Message::parse(&client, Box::new(m), &entities).facts(&me), "case {i}");
            assert_eq!(facts.flags & NEW, bits, "case {i}");
        }
    }

    #[test]
    fn chats_acting_as_senders_set_sender_chat() {
        let (client, entities, me) = (Client::offline(), entities(), Me::default());
        let group = get_peer(get_channel_id(55));
        let post = edited(|m| (m.peer_id, m.post) = (get_peer(get_channel_id(56)), true));
        let anonymous = edited(|m| (m.peer_id, m.from_id) = (group.clone(), Some(group.clone())));
        let user = edited(|m| (m.peer_id, m.from_id) = (group.clone(), Some(get_peer(8))));
        for (m, sender_chat) in [(post, true), (anonymous, true), (user, false)] {
            let raw = base::Message::Message(Box::new(m.clone()));
            let facts = Facts::message(&raw, PeerKinds::message(&raw, &entities), &me);
            assert_eq!(facts, Message::parse(&client, Box::new(m), &entities).facts(&me));
            assert_eq!(facts.flags & SENDER_CHAT != 0, sender_chat);
        }
        let reaction = |actor| types::UpdateBotMessageReaction {
            peer: group.clone(),
            msg_id: 10,
            date: 1,
            actor,
            old_reactions: vec![],
            new_reactions: vec![],
            qts: 1,
        };
        let by_chat = MessageReactionUpdated::parse(&client, &reaction(group.clone()), &entities);
        let by_user = MessageReactionUpdated::parse(&client, &reaction(get_peer(8)), &entities);
        assert_ne!(by_chat.facts(&me).flags & SENDER_CHAT, 0);
        assert_eq!(by_user.facts(&me).flags & SENDER_CHAT, 0);
    }

    fn unique_star_gift() -> base::StarGift {
        base::StarGift::StarGiftUnique(Box::new(types::StarGiftUnique {
            require_premium: false,
            resale_ton_only: false,
            theme_available: false,
            burned: false,
            crafted: false,
            id: 1,
            gift_id: 2,
            title: "Plush Pepe".into(),
            slug: "PlushPepe-3".into(),
            num: 3,
            owner_id: None,
            owner_name: None,
            owner_address: None,
            attributes: Vec::new(),
            availability_issued: 10,
            availability_total: 100,
            gift_address: None,
            resell_amount: None,
            released_by: None,
            value_amount: None,
            value_currency: None,
            value_usd_amount: None,
            theme_peer: None,
            peer_color: None,
            host_id: None,
            offer_min_stars: None,
            craft_chance_permille: None,
        }))
    }

    fn offer(accepted: bool, declined: bool) -> types::MessageService {
        let price = base::StarsAmount::StarsAmount(Box::new(types::StarsAmount { amount: 5, nanos: 0 }));
        let offer = types::MessageActionStarGiftPurchaseOffer {
            accepted,
            declined,
            gift: unique_star_gift(),
            price,
            expires_at: 9,
        };
        service(base::MessageAction::MessageActionStarGiftPurchaseOffer(Box::new(offer)))
    }

    #[test]
    fn new_bits_match_raw_and_parsed() {
        let (client, entities) = (Client::offline(), entities());
        let me = Me { id: 0, is_bot: true, username: Some("Bot".into()), ..Me::default() };
        let cases = [
            (false, false, 0),
            (true, false, OFFER_ACCEPTED),
            (false, true, OFFER_REJECTED),
            (true, true, OFFER_ACCEPTED),
        ];
        for (i, (accepted, declined, bit)) in cases.into_iter().enumerate() {
            let raw = base::Message::MessageService(Box::new(offer(accepted, declined)));
            let facts = Facts::message(&raw, PeerKinds::message(&raw, &entities), &me);
            let parsed = Message::from_raw(&client, raw.clone(), &entities).unwrap();
            assert_eq!(facts, parsed.facts(&me), "case {i}");
            let seen = (facts.service, facts.flags & (OFFER_ACCEPTED | OFFER_REJECTED));
            assert_eq!(seen, (Some(ServiceKind::UpgradedGiftPurchaseOffer), bit), "case {i}");
        }
    }

    #[test]
    fn scheduled_is_only_unsent_messages() {
        let (client, entities) = (Client::offline(), entities());
        let me = Me::default();
        let both = FROM_SCHEDULED | SCHEDULED;
        let mut sent = Message::bare(3, get_peer(8), 0);
        sent.from_scheduled = true;
        let raw = base::Message::Message(Box::new(sent.clone()));
        let facts = Facts::message(&raw, PeerKinds::message(&raw, &entities), &me);
        let parsed = Message::parse(&client, Box::new(sent), &entities);
        assert_eq!(facts, parsed.facts(&me));
        assert_eq!(facts.flags & both, FROM_SCHEDULED, "sent on schedule isn't unsent");

        let mut waiting = Message::parse(&client, Box::new(Message::bare(3, get_peer(8), 0)), &entities);
        assert_eq!(waiting.facts(&me).flags & both, 0);
        waiting.scheduled = true;
        assert_eq!(waiting.facts(&me).flags & both, SCHEDULED);
    }
}
