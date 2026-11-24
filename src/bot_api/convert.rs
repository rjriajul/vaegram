//! Bot API JSON to the TL Telegram sends an MTProto client, and the TL of requests back to Bot API JSON.
//!
//! What it builds is synthetic: users and chats come as `min`, with no access hash; files carry the ids their
//! `file_id` decodes to; fields the Bot API doesn't carry (pts, views, reactions, ...) stay empty.

use serde_json::{Map, Value, json};

use super::bridge::unsupported;
use super::raw::types as bot;
use crate::errors::Result;
use crate::file_id::{FileId, ThumbnailSource};
use crate::raw::{base, types};
use crate::types::{ButtonStyle, InlineMessageId};
use crate::utils::{CHANNEL_OFFSET, channel_id_of, get_chat_id, get_peer, input_sticker_set};

/// A Bot API integer as a TL `int`, clamped rather than wrapped.
pub(crate) fn int(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// The users and chats an update names, as the TL `users` and `chats` of an `Updates` carry them.
pub(crate) struct Peers {
    me: i64,
    pub(crate) users: Vec<base::User>,
    pub(crate) chats: Vec<base::Chat>,
}

impl Peers {
    /// `me` is the bot's own id: its user is marked as the account itself.
    pub(crate) fn new(me: i64) -> Peers {
        Peers { me, users: Vec::new(), chats: Vec::new() }
    }

    pub(crate) fn user(&mut self, u: &bot::User) {
        if self.has_user(u.id) {
            return;
        }
        let mut user = types::User::new(u.id);
        user.is_self = u.id == self.me;
        user.min = true;
        user.bot = u.is_bot;
        user.premium = u.is_premium.unwrap_or(false);
        user.first_name = Some(u.first_name.clone());
        user.last_name = u.last_name.clone();
        user.username = u.username.clone();
        user.lang_code = u.language_code.clone();
        self.users.push(base::User::User(Box::new(user)));
    }

    /// A private chat is the user on the other side, who may already be there as a sender with more detail.
    pub(crate) fn chat(&mut self, c: &bot::Chat) {
        match chat(c, false) {
            Some(chat) if !self.chats.iter().any(|known| get_chat_id(known) == Some(c.id)) => self.chats.push(chat),
            Some(_) => {}
            None if self.has_user(c.id) => {}
            None => self.users.push(chat_user(c, false)),
        }
    }

    fn has_user(&self, id: i64) -> bool {
        self.users.iter().any(|u| matches!(u, base::User::User(u) if u.id == id))
    }
}

/// The user behind a private chat. `resolved` gives it an access hash of 0, which the Bot API never needs, so
/// requests can name it; otherwise it is `min`, as a user seen in an update is.
pub(crate) fn chat_user(c: &bot::Chat, resolved: bool) -> base::User {
    let mut user = types::User::new(c.id);
    user.min = !resolved;
    user.access_hash = resolved.then_some(0);
    user.first_name = c.first_name.clone();
    user.last_name = c.last_name.clone();
    user.username = c.username.clone();
    base::User::User(Box::new(user))
}

/// The TL chat of a group, supergroup or channel; `None` for a private chat. `resolved` as for [`chat_user`].
pub(crate) fn chat(c: &bot::Chat, resolved: bool) -> Option<base::Chat> {
    if c.id > 0 {
        return None;
    }
    let title = c.title.clone().unwrap_or_default();
    if c.id > -CHANNEL_OFFSET {
        let chat = types::Chat::new(c.id.wrapping_neg(), title, base::ChatPhoto::ChatPhotoEmpty, 0, 0, 0);
        return Some(base::Chat::Chat(Box::new(chat)));
    }
    let mut channel = types::Channel::new(channel_id_of(c.id), title, base::ChatPhoto::ChatPhotoEmpty, 0);
    channel.broadcast = c.r#type == "channel";
    channel.megagroup = !channel.broadcast;
    channel.forum = c.is_forum.unwrap_or(false);
    channel.monoforum = c.is_direct_messages.unwrap_or(false);
    channel.username = c.username.clone();
    channel.min = !resolved;
    channel.access_hash = resolved.then_some(0);
    Some(base::Chat::Channel(Box::new(channel)))
}

/// A message, with the users and chats it names added to `peers`.
pub(crate) fn message(m: bot::Message, peers: &mut Peers) -> types::Message {
    let post = m.chat.r#type == "channel";
    if let Some(from) = &m.from {
        peers.user(from);
    }
    peers.chat(&m.chat);
    if let Some(chat) = &m.sender_chat {
        peers.chat(chat);
    }
    // A channel post has no sender; a message sent on behalf of a chat names the chat.
    let from_id = if post {
        None
    } else if let Some(chat) = &m.sender_chat {
        Some(get_peer(chat.id))
    } else {
        m.from.as_ref().map(|u| get_peer(u.id))
    };
    let out = m.from.as_ref().is_some_and(|u| u.id == peers.me);
    let media = media(&m);
    let reply_to = reply_header(&m, peers);
    let fwd_from = m.forward_origin.as_ref().map(|origin| fwd_header(origin, peers));
    let via_bot_id = m.via_bot.as_ref().map(|bot| {
        peers.user(bot);
        bot.id
    });
    let (text, entities) = match m.text {
        Some(text) => (text, m.entities),
        None => (m.caption.unwrap_or_default(), m.caption_entities),
    };
    let mut raw = types::Message::new(int(m.message_id), get_peer(m.chat.id), int(m.date), text);
    raw.out = out;
    raw.post = post;
    raw.from_id = from_id;
    raw.media = media;
    raw.reply_to = reply_to;
    raw.fwd_from = fwd_from;
    raw.via_bot_id = via_bot_id;
    raw.entities = entities.map(|list| self::entities(&list, peers)).filter(|list| !list.is_empty());
    raw.reply_markup = m.reply_markup.map(inline_markup);
    raw.edit_date = m.edit_date.map(int);
    raw.grouped_id = m.media_group_id.and_then(|id| id.parse().ok());
    raw.post_author = m.author_signature;
    raw.noforwards = m.has_protected_content.unwrap_or(false);
    raw.offline = m.is_from_offline.unwrap_or(false);
    raw.invert_media = m.show_caption_above_media.unwrap_or(false);
    raw.from_boosts_applied = m.sender_boost_count.map(int);
    raw.from_rank = m.sender_tag;
    raw.effect = m.effect_id.and_then(|id| id.parse().ok());
    raw.paid_message_stars = m.paid_star_count;
    raw
}

/// The update a new or edited message comes in, with a pts of 0: the Bot API numbers nothing, and the dispatcher
/// hands unnumbered updates straight on.
pub(crate) fn message_update(raw: types::Message, edit: bool) -> base::Update {
    let channel = matches!(raw.peer_id, base::Peer::PeerChannel(_));
    let message = base::Message::Message(Box::new(raw));
    match (channel, edit) {
        (false, false) => {
            base::Update::UpdateNewMessage(Box::new(types::UpdateNewMessage { message, pts: 0, pts_count: 0 }))
        }
        (true, false) => base::Update::UpdateNewChannelMessage(Box::new(types::UpdateNewChannelMessage {
            message,
            pts: 0,
            pts_count: 0,
        })),
        (false, true) => {
            base::Update::UpdateEditMessage(Box::new(types::UpdateEditMessage { message, pts: 0, pts_count: 0 }))
        }
        (true, true) => base::Update::UpdateEditChannelMessage(Box::new(types::UpdateEditChannelMessage {
            message,
            pts: 0,
            pts_count: 0,
        })),
    }
}

/// The TL of one `getUpdates` update, for the kinds the client handles: messages and channel posts, new or edited,
/// callback queries and inline queries. `None` for the other kinds.
pub(crate) fn update(u: bot::Update, peers: &mut Peers) -> Option<base::Update> {
    if let Some(m) = u.message.or(u.channel_post) {
        return Some(message_update(message(*m, peers), false));
    }
    if let Some(m) = u.edited_message.or(u.edited_channel_post) {
        return Some(message_update(message(*m, peers), true));
    }
    if let Some(query) = u.callback_query {
        return callback_query(query, peers);
    }
    if let Some(query) = u.inline_query {
        return inline_query(query, peers);
    }
    None
}

/// `None` when the query or chat instance id isn't a number, as the Bot API always sends them, or the message is
/// of a kind this layer doesn't know.
pub(crate) fn callback_query(q: bot::CallbackQuery, peers: &mut Peers) -> Option<base::Update> {
    let query_id = q.id.parse().ok()?;
    let chat_instance = q.chat_instance.parse().unwrap_or(0);
    peers.user(&q.from);
    let data = q.data.map(String::into_bytes);
    if let Some(id) = q.inline_message_id {
        let msg_id = id.parse::<InlineMessageId>().ok()?.0;
        let mut update = types::UpdateInlineBotCallbackQuery::new(query_id, q.from.id, msg_id, chat_instance);
        (update.data, update.game_short_name) = (data, q.game_short_name);
        return Some(base::Update::UpdateInlineBotCallbackQuery(Box::new(update)));
    }
    let (chat, message_id) = match q.message? {
        bot::MaybeInaccessibleMessage::Message(m) => {
            peers.chat(&m.chat);
            (m.chat.id, m.message_id)
        }
        bot::MaybeInaccessibleMessage::InaccessibleMessage(m) => {
            peers.chat(&m.chat);
            (m.chat.id, m.message_id)
        }
        bot::MaybeInaccessibleMessage::Unknown(_) => return None,
    };
    let mut update =
        types::UpdateBotCallbackQuery::new(query_id, q.from.id, get_peer(chat), int(message_id), chat_instance);
    (update.data, update.game_short_name) = (data, q.game_short_name);
    Some(base::Update::UpdateBotCallbackQuery(Box::new(update)))
}

/// `None` when the query id isn't a number, as the Bot API always sends it.
pub(crate) fn inline_query(q: bot::InlineQuery, peers: &mut Peers) -> Option<base::Update> {
    let query_id = q.id.parse().ok()?;
    peers.user(&q.from);
    let mut update = types::UpdateBotInlineQuery::new(query_id, q.from.id, q.query, q.offset);
    let geo = |l: bot::Location| types::GeoPoint::new(l.longitude, l.latitude, 0);
    update.geo = q.location.map(|l| base::GeoPoint::GeoPoint(Box::new(geo(l))));
    update.peer_type = q.chat_type.as_deref().and_then(|kind| {
        use base::InlineQueryPeerType as P;
        Some(match kind {
            "sender" => P::InlineQueryPeerTypeSameBotPM,
            "private" => P::InlineQueryPeerTypePM,
            "group" => P::InlineQueryPeerTypeChat,
            "supergroup" => P::InlineQueryPeerTypeMegagroup,
            "channel" => P::InlineQueryPeerTypeBroadcast,
            _ => return None,
        })
    });
    Some(base::Update::UpdateBotInlineQuery(Box::new(update)))
}

fn reply_header(m: &bot::Message, peers: &mut Peers) -> Option<base::MessageReplyHeader> {
    let replied = m.reply_to_message.as_ref().map(|r| int(r.message_id));
    let topic = m.message_thread_id.map(int).filter(|_| m.is_topic_message.unwrap_or(false));
    let external = m.external_reply.as_deref();
    if replied.is_none() && topic.is_none() && external.is_none() {
        return None;
    }
    if let Some(from) = m.reply_to_message.as_ref().and_then(|r| r.from.as_ref()) {
        peers.user(from);
    }
    // In a topic, a message that replies to nothing replies to the topic's first message, as in MTProto.
    let reply_to_msg_id = replied.or_else(|| external.and_then(|e| e.message_id).map(int)).or(topic);
    let reply_to_peer_id = external.and_then(|e| e.chat.as_ref()).map(|chat| {
        peers.chat(chat);
        get_peer(chat.id)
    });
    let quote = m.quote.as_ref();
    let header = types::MessageReplyHeader {
        forum_topic: topic.is_some(),
        quote: quote.is_some_and(|q| q.is_manual.unwrap_or(false)),
        reply_to_msg_id,
        reply_to_peer_id,
        reply_from: external.map(|e| fwd_header(&e.origin, peers)),
        reply_to_top_id: topic.filter(|&top| reply_to_msg_id != Some(top)),
        quote_text: quote.map(|q| q.text.clone()),
        quote_entities: quote.and_then(|q| q.entities.as_ref()).map(|list| entities(list, peers)),
        quote_offset: quote.map(|q| int(q.position)),
        todo_item_id: m.reply_to_checklist_task_id.map(int),
        ..Default::default()
    };
    Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(header)))
}

fn fwd_header(origin: &bot::MessageOrigin, peers: &mut Peers) -> base::MessageFwdHeader {
    let (date, from_id, from_name, channel_post, post_author) = match origin {
        bot::MessageOrigin::User(o) => {
            peers.user(&o.sender_user);
            (o.date, Some(get_peer(o.sender_user.id)), None, None, None)
        }
        bot::MessageOrigin::HiddenUser(o) => (o.date, None, Some(o.sender_user_name.clone()), None, None),
        bot::MessageOrigin::Chat(o) => {
            peers.chat(&o.sender_chat);
            (o.date, Some(get_peer(o.sender_chat.id)), None, None, o.author_signature.clone())
        }
        bot::MessageOrigin::Channel(o) => {
            peers.chat(&o.chat);
            (o.date, Some(get_peer(o.chat.id)), None, Some(int(o.message_id)), o.author_signature.clone())
        }
        bot::MessageOrigin::Unknown => (0, None, None, None, None),
    };
    let mut header = types::MessageFwdHeader::new(int(date));
    (header.from_id, header.from_name, header.channel_post, header.post_author) =
        (from_id, from_name, channel_post, post_author);
    base::MessageFwdHeader::MessageFwdHeader(Box::new(header))
}

/// Entity kinds this layer has no TL for are left out.
pub(crate) fn entities(list: &[bot::MessageEntity], peers: &mut Peers) -> Vec<base::MessageEntity> {
    list.iter().filter_map(|e| entity(e, peers)).collect()
}

fn entity(e: &bot::MessageEntity, peers: &mut Peers) -> Option<base::MessageEntity> {
    use base::MessageEntity as E;
    let (offset, length) = (int(e.offset), int(e.length));
    Some(match e.r#type.as_str() {
        "mention" => E::MessageEntityMention(types::MessageEntityMention { offset, length }),
        "hashtag" => E::MessageEntityHashtag(types::MessageEntityHashtag { offset, length }),
        "cashtag" => E::MessageEntityCashtag(types::MessageEntityCashtag { offset, length }),
        "bot_command" => E::MessageEntityBotCommand(types::MessageEntityBotCommand { offset, length }),
        "url" => E::MessageEntityUrl(types::MessageEntityUrl { offset, length }),
        "email" => E::MessageEntityEmail(types::MessageEntityEmail { offset, length }),
        "phone_number" => E::MessageEntityPhone(types::MessageEntityPhone { offset, length }),
        "bold" => E::MessageEntityBold(types::MessageEntityBold { offset, length }),
        "italic" => E::MessageEntityItalic(types::MessageEntityItalic { offset, length }),
        "underline" => E::MessageEntityUnderline(types::MessageEntityUnderline { offset, length }),
        "strikethrough" => E::MessageEntityStrike(types::MessageEntityStrike { offset, length }),
        "spoiler" => E::MessageEntitySpoiler(types::MessageEntitySpoiler { offset, length }),
        "code" => E::MessageEntityCode(types::MessageEntityCode { offset, length }),
        "pre" => {
            let language = e.language.clone().unwrap_or_default();
            E::MessageEntityPre(Box::new(types::MessageEntityPre { offset, length, language }))
        }
        "text_link" => {
            let url = e.url.clone().unwrap_or_default();
            E::MessageEntityTextUrl(Box::new(types::MessageEntityTextUrl { offset, length, url }))
        }
        "text_mention" => {
            let user = e.user.as_ref()?;
            peers.user(user);
            E::MessageEntityMentionName(Box::new(types::MessageEntityMentionName { offset, length, user_id: user.id }))
        }
        "custom_emoji" => {
            let document_id = e.custom_emoji_id.as_deref()?.parse().ok()?;
            E::MessageEntityCustomEmoji(Box::new(types::MessageEntityCustomEmoji { offset, length, document_id }))
        }
        "blockquote" | "expandable_blockquote" => {
            let collapsed = e.r#type == "expandable_blockquote";
            E::MessageEntityBlockquote(Box::new(types::MessageEntityBlockquote { collapsed, offset, length }))
        }
        "date_time" => {
            let format = e.date_time_format.as_deref().unwrap_or_default();
            let has = |c| format.contains(c);
            E::MessageEntityFormattedDate(Box::new(types::MessageEntityFormattedDate {
                relative: has('r'),
                short_time: has('t'),
                long_time: has('T'),
                short_date: has('d'),
                long_date: has('D'),
                day_of_week: has('w'),
                offset,
                length,
                date: int(e.unix_time?),
            }))
        }
        _ => return None,
    })
}

/// A file a message carries, as its attributes describe it.
struct File<'a> {
    file_id: &'a str,
    mime_type: &'a str,
    size: Option<i64>,
    thumbnail: Option<&'a bot::PhotoSize>,
    attributes: Vec<base::DocumentAttribute>,
    video: bool,
    round: bool,
    voice: bool,
}

/// `None` for media of another kind, and for a file id that doesn't decode.
fn media(m: &bot::Message) -> Option<base::MessageMedia> {
    let (date, spoiler) = (int(m.date), m.has_media_spoiler.unwrap_or(false));
    if let Some(sizes) = &m.photo {
        return photo(sizes, date, spoiler);
    }
    let file = if let Some(a) = &m.animation {
        let video = video_attribute(a.duration, a.width, a.height, false);
        let attributes = named(vec![video, base::DocumentAttribute::DocumentAttributeAnimated], &a.file_name);
        let mime_type = a.mime_type.as_deref().unwrap_or("video/mp4");
        file(&a.file_id, mime_type, a.file_size, a.thumbnail.as_ref(), attributes)
    } else if let Some(v) = &m.video {
        let attributes = named(vec![video_attribute(v.duration, v.width, v.height, false)], &v.file_name);
        let mime_type = v.mime_type.as_deref().unwrap_or("video/mp4");
        File { video: true, ..file(&v.file_id, mime_type, v.file_size, v.thumbnail.as_ref(), attributes) }
    } else if let Some(v) = &m.video_note {
        let attributes = vec![video_attribute(v.duration, v.length, v.length, true)];
        File { round: true, ..file(&v.file_id, "video/mp4", v.file_size, v.thumbnail.as_ref(), attributes) }
    } else if let Some(a) = &m.audio {
        let audio = audio_attribute(false, a.duration, a.title.clone(), a.performer.clone());
        let mime_type = a.mime_type.as_deref().unwrap_or("audio/mpeg");
        file(&a.file_id, mime_type, a.file_size, a.thumbnail.as_ref(), named(vec![audio], &a.file_name))
    } else if let Some(v) = &m.voice {
        let attributes = vec![audio_attribute(true, v.duration, None, None)];
        let mime_type = v.mime_type.as_deref().unwrap_or("audio/ogg");
        File { voice: true, ..file(&v.file_id, mime_type, v.file_size, None, attributes) }
    } else if let Some(s) = &m.sticker {
        let sticker = types::DocumentAttributeSticker {
            mask: s.r#type == "mask",
            alt: s.emoji.clone().unwrap_or_default(),
            stickerset: s.set_name.as_deref().map_or(base::InputStickerSet::InputStickerSetEmpty, input_sticker_set),
            mask_coords: None,
        };
        let size = types::DocumentAttributeImageSize { w: int(s.width), h: int(s.height) };
        let attributes = vec![
            base::DocumentAttribute::DocumentAttributeSticker(Box::new(sticker)),
            base::DocumentAttribute::DocumentAttributeImageSize(size),
        ];
        let mime_type = match (s.is_animated, s.is_video) {
            (true, _) => "application/x-tgsticker",
            (_, true) => "video/webm",
            _ => "image/webp",
        };
        file(&s.file_id, mime_type, s.file_size, s.thumbnail.as_ref(), attributes)
    } else if let Some(d) = &m.document {
        let mime_type = d.mime_type.as_deref().unwrap_or("application/octet-stream");
        file(&d.file_id, mime_type, d.file_size, d.thumbnail.as_ref(), named(Vec::new(), &d.file_name))
    } else {
        return None;
    };
    document(file, date, spoiler)
}

fn file<'a>(
    file_id: &'a str,
    mime_type: &'a str,
    size: Option<i64>,
    thumbnail: Option<&'a bot::PhotoSize>,
    attributes: Vec<base::DocumentAttribute>,
) -> File<'a> {
    File { file_id, mime_type, size, thumbnail, attributes, video: false, round: false, voice: false }
}

fn named(mut attributes: Vec<base::DocumentAttribute>, file_name: &Option<String>) -> Vec<base::DocumentAttribute> {
    if let Some(file_name) = file_name {
        let name = types::DocumentAttributeFilename { file_name: file_name.clone() };
        attributes.push(base::DocumentAttribute::DocumentAttributeFilename(Box::new(name)));
    }
    attributes
}

fn video_attribute(duration: i64, w: i64, h: i64, round_message: bool) -> base::DocumentAttribute {
    base::DocumentAttribute::DocumentAttributeVideo(Box::new(types::DocumentAttributeVideo {
        round_message,
        supports_streaming: false,
        nosound: false,
        duration: duration as f64,
        w: int(w),
        h: int(h),
        preload_prefix_size: None,
        video_start_ts: None,
        video_codec: None,
    }))
}

fn audio_attribute(
    voice: bool,
    duration: i64,
    title: Option<String>,
    performer: Option<String>,
) -> base::DocumentAttribute {
    let audio = types::DocumentAttributeAudio { voice, duration: int(duration), title, performer, waveform: None };
    base::DocumentAttribute::DocumentAttributeAudio(Box::new(audio))
}

fn document(file: File<'_>, date: i32, spoiler: bool) -> Option<base::MessageMedia> {
    let id = FileId::decode(file.file_id).ok()?;
    let size = file.size.unwrap_or(0);
    let mut document = types::Document::new(
        id.media_id,
        id.access_hash,
        id.file_reference,
        date,
        file.mime_type.into(),
        size,
        id.dc_id,
        file.attributes,
    );
    document.thumbs = file.thumbnail.and_then(photo_size).map(|size| vec![size]);
    let media = types::MessageMediaDocument {
        document: Some(base::Document::Document(Box::new(document))),
        video: file.video,
        round: file.round,
        voice: file.voice,
        spoiler,
        ..Default::default()
    };
    Some(base::MessageMedia::MessageMediaDocument(Box::new(media)))
}

fn photo(sizes: &[bot::PhotoSize], date: i32, spoiler: bool) -> Option<base::MessageMedia> {
    let id = FileId::decode(&sizes.last()?.file_id).ok()?;
    let sizes = sizes.iter().filter_map(photo_size).collect();
    let photo = types::Photo::new(id.media_id, id.access_hash, id.file_reference, date, sizes, id.dc_id);
    let media =
        types::MessageMediaPhoto { photo: Some(base::Photo::Photo(Box::new(photo))), spoiler, ..Default::default() };
    Some(base::MessageMedia::MessageMediaPhoto(Box::new(media)))
}

/// One size, named by the letter its file id carries, or by its width when the id has none.
fn photo_size(photo: &bot::PhotoSize) -> Option<base::PhotoSize> {
    let letter = match FileId::decode(&photo.file_id).ok()?.thumbnail_source {
        Some(ThumbnailSource::Thumbnail { size, .. }) => size,
        _ => [(100, 's'), (320, 'm'), (800, 'x'), (1280, 'y')]
            .into_iter()
            .find(|&(max, _)| photo.width <= max)
            .map_or('w', |(_, letter)| letter),
    };
    Some(base::PhotoSize::PhotoSize(Box::new(types::PhotoSize {
        r#type: letter.to_string(),
        w: int(photo.width),
        h: int(photo.height),
        size: photo.file_size.map_or(0, int),
    })))
}

fn inline_markup(markup: bot::InlineKeyboardMarkup) -> base::ReplyMarkup {
    let rows = markup
        .inline_keyboard
        .into_iter()
        .map(|row| {
            let buttons = row.into_iter().map(inline_button).collect();
            base::KeyboardInlineButtonRow::KeyboardInlineButtonRow(Box::new(types::KeyboardInlineButtonRow { buttons }))
        })
        .collect();
    let markup = types::ReplyInlineMarkup { force_reply: markup.force_reply.unwrap_or(false), rows };
    base::ReplyMarkup::ReplyInlineMarkup(Box::new(markup))
}

/// A login button comes as a plain link, as the Bot API itself shows it; a button of a kind added later as a disabled
/// one, so the layout stays.
fn inline_button(b: bot::InlineKeyboardButton) -> base::KeyboardInlineButton {
    use base::InlineButtonType as T;
    let switch = |query: String, same_peer: bool, peer_types: Option<Vec<base::InlineQueryPeerType>>| {
        T::InlineButtonTypeSwitchInline(Box::new(types::InlineButtonTypeSwitchInline { same_peer, query, peer_types }))
    };
    let kind = if let Some(data) = b.callback_data {
        T::InlineButtonTypeCallback(Box::new(types::InlineButtonTypeCallback {
            requires_password: false,
            data: data.into_bytes(),
        }))
    } else if let Some(url) = b.url.or(b.login_url.map(|login| login.url)) {
        T::InlineButtonTypeUrl(Box::new(types::InlineButtonTypeUrl { url }))
    } else if let Some(app) = b.web_app {
        T::InlineButtonTypeWebView(Box::new(types::InlineButtonTypeWebView { url: app.url }))
    } else if let Some(query) = b.switch_inline_query {
        switch(query, false, None)
    } else if let Some(query) = b.switch_inline_query_current_chat {
        switch(query, true, None)
    } else if let Some(chosen) = b.switch_inline_query_chosen_chat {
        use base::InlineQueryPeerType as P;
        let allowed = [
            (chosen.allow_user_chats, P::InlineQueryPeerTypePM),
            (chosen.allow_bot_chats, P::InlineQueryPeerTypeBotPM),
            (chosen.allow_group_chats, P::InlineQueryPeerTypeChat),
            (chosen.allow_group_chats, P::InlineQueryPeerTypeMegagroup),
            (chosen.allow_channel_chats, P::InlineQueryPeerTypeBroadcast),
        ];
        let peer_types = allowed.into_iter().filter(|(on, _)| *on == Some(true)).map(|(_, kind)| kind).collect();
        switch(chosen.query.unwrap_or_default(), false, Some(peer_types))
    } else if let Some(copy) = b.copy_text {
        T::InlineButtonTypeCopy(Box::new(types::InlineButtonTypeCopy { copy_text: copy.text }))
    } else if b.callback_game.is_some() {
        T::InlineButtonTypeGame
    } else if b.pay == Some(true) {
        T::InlineButtonTypeBuy
    } else {
        T::InlineButtonTypeDisabled
    };
    let style = match b.style.as_deref() {
        Some("primary") => ButtonStyle::Primary,
        Some("danger") => ButtonStyle::Danger,
        Some("success") => ButtonStyle::Success,
        _ => ButtonStyle::Default,
    };
    let icon = b.icon_custom_emoji_id.as_deref().and_then(|id| id.parse().ok());
    let button = types::KeyboardInlineButton { style: style.write_with_icon(icon), text: b.text, r#type: kind };
    base::KeyboardInlineButton::KeyboardInlineButton(Box::new(button))
}

/// The user an input user names; `me` stands for `inputUserSelf`.
pub(crate) fn input_user_id(user: &base::InputUser, me: i64) -> Option<i64> {
    match user {
        base::InputUser::InputUser(u) => Some(u.user_id),
        base::InputUser::InputUserFromMessage(u) => Some(u.user_id),
        base::InputUser::InputUserSelf => Some(me),
        base::InputUser::InputUserEmpty => None,
    }
}

/// The Bot API entities of TL ones; kinds the Bot API has no name for (bank cards, TON addresses, diffs) are left
/// out.
pub(crate) fn entities_json(list: &[base::MessageEntity], me: i64) -> Value {
    Value::Array(list.iter().filter_map(|e| entity_json(e, me)).collect())
}

fn entity_json(e: &base::MessageEntity, me: i64) -> Option<Value> {
    use base::MessageEntity as E;
    let mut out = Map::new();
    let (kind, offset, length) = match e {
        E::MessageEntityMention(e) => ("mention", e.offset, e.length),
        E::MessageEntityHashtag(e) => ("hashtag", e.offset, e.length),
        E::MessageEntityCashtag(e) => ("cashtag", e.offset, e.length),
        E::MessageEntityBotCommand(e) => ("bot_command", e.offset, e.length),
        E::MessageEntityUrl(e) => ("url", e.offset, e.length),
        E::MessageEntityEmail(e) => ("email", e.offset, e.length),
        E::MessageEntityPhone(e) => ("phone_number", e.offset, e.length),
        E::MessageEntityBold(e) => ("bold", e.offset, e.length),
        E::MessageEntityItalic(e) => ("italic", e.offset, e.length),
        E::MessageEntityUnderline(e) => ("underline", e.offset, e.length),
        E::MessageEntityStrike(e) => ("strikethrough", e.offset, e.length),
        E::MessageEntitySpoiler(e) => ("spoiler", e.offset, e.length),
        E::MessageEntityCode(e) => ("code", e.offset, e.length),
        E::MessageEntityPre(e) => {
            if !e.language.is_empty() {
                out.insert("language".into(), e.language.clone().into());
            }
            ("pre", e.offset, e.length)
        }
        E::MessageEntityTextUrl(e) => {
            out.insert("url".into(), e.url.clone().into());
            ("text_link", e.offset, e.length)
        }
        E::MessageEntityMentionName(e) => {
            out.insert("user".into(), mentioned(e.user_id));
            ("text_mention", e.offset, e.length)
        }
        E::InputMessageEntityMentionName(e) => {
            out.insert("user".into(), mentioned(input_user_id(&e.user_id, me)?));
            ("text_mention", e.offset, e.length)
        }
        E::MessageEntityCustomEmoji(e) => {
            out.insert("custom_emoji_id".into(), e.document_id.to_string().into());
            ("custom_emoji", e.offset, e.length)
        }
        E::MessageEntityBlockquote(e) if e.collapsed => ("expandable_blockquote", e.offset, e.length),
        E::MessageEntityBlockquote(e) => ("blockquote", e.offset, e.length),
        E::MessageEntityFormattedDate(e) => {
            out.insert("unix_time".into(), e.date.into());
            let format: String = if e.relative {
                "r".into()
            } else {
                [
                    (e.day_of_week, 'w'),
                    (e.long_date, 'D'),
                    (e.short_date && !e.long_date, 'd'),
                    (e.long_time, 'T'),
                    (e.short_time && !e.long_time, 't'),
                ]
                .into_iter()
                .filter_map(|(set, letter)| set.then_some(letter))
                .collect()
            };
            if !format.is_empty() {
                out.insert("date_time_format".into(), format.into());
            }
            ("date_time", e.offset, e.length)
        }
        _ => return None,
    };
    out.insert("type".into(), kind.into());
    out.insert("offset".into(), offset.into());
    out.insert("length".into(), length.into());
    Some(Value::Object(out))
}

/// A text mention names its user by id; the Bot API reads nothing else of it.
fn mentioned(user_id: i64) -> Value {
    json!({ "id": user_id, "is_bot": false, "first_name": "" })
}

/// The Bot API keyboard of a TL one. Buttons that ask for users or chats, and callback buttons behind the cloud
/// password, have no Bot API form.
pub(crate) fn markup_json(markup: &base::ReplyMarkup, me: i64) -> Result<Value> {
    let mut out = Map::new();
    match markup {
        base::ReplyMarkup::ReplyInlineMarkup(m) => {
            let mut rows = Vec::with_capacity(m.rows.len());
            for base::KeyboardInlineButtonRow::KeyboardInlineButtonRow(row) in &m.rows {
                let buttons = row.buttons.iter().map(|b| inline_button_json(b, me)).collect::<Result<Vec<_>>>()?;
                rows.push(Value::Array(buttons));
            }
            out.insert("inline_keyboard".into(), Value::Array(rows));
            if m.force_reply {
                out.insert("force_reply".into(), true.into());
            }
        }
        base::ReplyMarkup::ReplyKeyboardMarkup(m) => {
            let mut rows = Vec::with_capacity(m.rows.len());
            for base::KeyboardButtonRow::KeyboardButtonRow(row) in &m.rows {
                let buttons = row.buttons.iter().map(keyboard_button_json).collect::<Result<Vec<_>>>()?;
                rows.push(Value::Array(buttons));
            }
            out.insert("keyboard".into(), Value::Array(rows));
            out.insert("resize_keyboard".into(), m.resize.into());
            out.insert("one_time_keyboard".into(), m.single_use.into());
            out.insert("is_persistent".into(), m.persistent.into());
            out.insert("selective".into(), m.selective.into());
            if let Some(placeholder) = &m.placeholder {
                out.insert("input_field_placeholder".into(), placeholder.clone().into());
            }
        }
        base::ReplyMarkup::ReplyKeyboardHide(m) => {
            out.insert("remove_keyboard".into(), true.into());
            out.insert("selective".into(), m.selective.into());
        }
        base::ReplyMarkup::ReplyKeyboardForceReply(m) => {
            out.insert("force_reply".into(), true.into());
            out.insert("selective".into(), m.selective.into());
            if let Some(placeholder) = &m.placeholder {
                out.insert("input_field_placeholder".into(), placeholder.clone().into());
            }
        }
    }
    Ok(Value::Object(out))
}

fn style_json(style: Option<&base::KeyboardButtonStyle>, out: &mut Map<String, Value>) {
    if let Some(base::KeyboardButtonStyle::KeyboardButtonStyle(s)) = style {
        let name = match (s.bg_primary, s.bg_danger, s.bg_success) {
            (true, _, _) => Some("primary"),
            (_, true, _) => Some("danger"),
            (_, _, true) => Some("success"),
            _ => None,
        };
        if let Some(name) = name {
            out.insert("style".into(), name.into());
        }
        if let Some(icon) = s.icon {
            out.insert("icon_custom_emoji_id".into(), icon.to_string().into());
        }
    }
}

fn inline_button_json(button: &base::KeyboardInlineButton, me: i64) -> Result<Value> {
    use base::InlineButtonType as T;
    let base::KeyboardInlineButton::KeyboardInlineButton(b) = button;
    let mut out = Map::new();
    out.insert("text".into(), b.text.clone().into());
    style_json(b.style.as_ref(), &mut out);
    let (key, value) = match &b.r#type {
        T::InlineButtonTypeUrl(t) => ("url", t.url.clone().into()),
        T::InlineButtonTypeCallback(t) if t.requires_password => {
            return Err(unsupported("callback buttons behind the cloud password"));
        }
        T::InlineButtonTypeCallback(t) => ("callback_data", String::from_utf8_lossy(&t.data).into_owned().into()),
        T::InlineButtonTypeWebView(t) => ("web_app", json!({ "url": t.url })),
        T::InlineButtonTypeUrlAuth(t) => ("login_url", json!({ "url": t.url, "forward_text": t.fwd_text })),
        T::InputInlineButtonTypeUrlAuth(t) => {
            let write = t.request_write_access;
            ("login_url", json!({ "url": t.url, "forward_text": t.fwd_text, "request_write_access": write }))
        }
        T::InlineButtonTypeSwitchInline(t) if t.same_peer => {
            ("switch_inline_query_current_chat", t.query.clone().into())
        }
        T::InlineButtonTypeSwitchInline(t) => match &t.peer_types {
            None => ("switch_inline_query", t.query.clone().into()),
            Some(kinds) => {
                use base::InlineQueryPeerType as P;
                let mut chosen = Map::new();
                chosen.insert("query".into(), t.query.clone().into());
                for kind in kinds {
                    let key = match kind {
                        P::InlineQueryPeerTypePM => "allow_user_chats",
                        P::InlineQueryPeerTypeBotPM => "allow_bot_chats",
                        P::InlineQueryPeerTypeChat | P::InlineQueryPeerTypeMegagroup => "allow_group_chats",
                        P::InlineQueryPeerTypeBroadcast => "allow_channel_chats",
                        _ => continue,
                    };
                    chosen.insert(key.into(), true.into());
                }
                ("switch_inline_query_chosen_chat", Value::Object(chosen))
            }
        },
        T::InputInlineButtonTypeUserProfile(t) => {
            let id = input_user_id(&t.user_id, me).ok_or_else(|| unsupported("profile buttons without a user"))?;
            ("url", format!("tg://user?id={id}").into())
        }
        T::InlineButtonTypeCopy(t) => ("copy_text", json!({ "text": t.copy_text })),
        T::InlineButtonTypeGame => ("callback_game", json!({})),
        T::InlineButtonTypeBuy => ("pay", true.into()),
        T::InlineButtonTypeDisabled => ("disabled", json!({})),
        _ => return Err(unsupported("this kind of inline button")),
    };
    out.insert(key.into(), value);
    Ok(Value::Object(out))
}

fn keyboard_button_json(button: &base::KeyboardButton) -> Result<Value> {
    use base::ButtonType as T;
    let base::KeyboardButton::KeyboardButton(b) = button;
    let mut out = Map::new();
    out.insert("text".into(), b.text.clone().into());
    style_json(b.style.as_ref(), &mut out);
    match &b.r#type {
        T::ButtonTypeDefault => {}
        T::ButtonTypeRequestPhone => {
            out.insert("request_contact".into(), true.into());
        }
        T::ButtonTypeRequestGeoLocation => {
            out.insert("request_location".into(), true.into());
        }
        T::ButtonTypeRequestPoll(p) => {
            let poll = match p.quiz {
                Some(true) => json!({ "type": "quiz" }),
                Some(false) => json!({ "type": "regular" }),
                None => json!({}),
            };
            out.insert("request_poll".into(), poll);
        }
        T::ButtonTypeSimpleWebView(w) => {
            out.insert("web_app".into(), json!({ "url": w.url }));
        }
        _ => return Err(unsupported("keyboard buttons that ask for users or chats")),
    }
    Ok(Value::Object(out))
}

/// The Bot API form of an inline query result sent by link or as text. Results by file (`inputBotInlineResultPhoto`,
/// `...Document`) would need a Bot API file id, which a TL input file doesn't carry.
pub(crate) fn inline_result_json(result: &base::InputBotInlineResult, me: i64) -> Result<Value> {
    use base::InputBotInlineMessage as M;
    let base::InputBotInlineResult::InputBotInlineResult(r) = result else {
        return Err(unsupported("inline results sent by file"));
    };
    let mut out = Map::new();
    out.insert("id".into(), r.id.clone().into());
    let (content, caption, markup) = match &r.send_message {
        M::InputBotInlineMessageText(m) => {
            let mut content = Map::new();
            content.insert("message_text".into(), m.message.clone().into());
            if let Some(list) = &m.entities {
                content.insert("entities".into(), entities_json(list, me));
            }
            if m.no_webpage || m.invert_media {
                let preview = json!({ "is_disabled": m.no_webpage, "show_above_text": m.invert_media });
                content.insert("link_preview_options".into(), preview);
            }
            (Some(Value::Object(content)), None, m.reply_markup.as_ref())
        }
        M::InputBotInlineMessageMediaAuto(m) => {
            (None, Some((&m.message, m.entities.as_deref())), m.reply_markup.as_ref())
        }
        _ => return Err(unsupported("inline results with this kind of message")),
    };
    let url = |document: &Option<base::InputWebDocument>| {
        document.as_ref().map(|document| {
            let base::InputWebDocument::InputWebDocument(d) = document;
            (d.url.clone(), d.mime_type.clone())
        })
    };
    let (content_url, mime_type) = url(&r.content).unzip();
    let thumbnail = url(&r.thumb).map(|(url, _)| url).or_else(|| content_url.clone());
    let kind = match (r.r#type.as_str(), mime_type.as_deref()) {
        ("article", _) => "article",
        ("photo", _) => "photo",
        ("gif", Some("video/mp4")) => "mpeg4_gif",
        ("gif", _) => "gif",
        ("video", _) => "video",
        ("audio", _) => "audio",
        ("voice", _) => "voice",
        ("file", _) | ("document", _) => "document",
        _ => return Err(unsupported("this kind of inline result")),
    };
    out.insert("type".into(), kind.into());
    let content_key = match kind {
        "article" => None,
        "photo" => Some("photo_url"),
        "mpeg4_gif" => Some("mpeg4_url"),
        "gif" => Some("gif_url"),
        "video" => Some("video_url"),
        "audio" => Some("audio_url"),
        "voice" => Some("voice_url"),
        _ => Some("document_url"),
    };
    if let (Some(key), Some(url)) = (content_key, content_url) {
        out.insert(key.into(), url.into());
    }
    if let (true, Some(mime_type)) = (matches!(kind, "video" | "document"), mime_type) {
        out.insert("mime_type".into(), mime_type.into());
    }
    if let (false, Some(thumbnail)) = (matches!(kind, "audio" | "voice"), thumbnail) {
        out.insert("thumbnail_url".into(), thumbnail.into());
    }
    if let Some(title) = &r.title {
        out.insert("title".into(), title.clone().into());
    }
    if let Some(description) = &r.description {
        out.insert("description".into(), description.clone().into());
    }
    if let (Some(url), "article") = (&r.url, kind) {
        out.insert("url".into(), url.clone().into());
    }
    if let Some(content) = content {
        out.insert("input_message_content".into(), content);
    }
    if let Some((caption, list)) = caption.filter(|(caption, _)| !caption.is_empty()) {
        out.insert("caption".into(), caption.clone().into());
        if let Some(list) = list {
            out.insert("caption_entities".into(), entities_json(list, me));
        }
    }
    if let Some(markup) = markup {
        out.insert("reply_markup".into(), markup_json(markup, me)?);
    }
    Ok(Value::Object(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_json(id: i64) -> Value {
        json!({ "id": id, "is_bot": false, "first_name": "Ann", "username": "ann" })
    }

    fn parse_message(value: Value, me: i64) -> (types::Message, Peers) {
        let parsed: bot::Message = serde_json::from_value(value).unwrap();
        let mut peers = Peers::new(me);
        (message(parsed, &mut peers), peers)
    }

    #[test]
    fn text_message_keeps_sender_entities_reply_and_keyboard() {
        let (raw, peers) = parse_message(
            json!({
                "message_id": 10,
                "date": 1_700_000_000,
                "from": user_json(7),
                "chat": { "id": 7, "type": "private", "first_name": "Ann" },
                "text": "/start now",
                "entities": [{ "type": "bot_command", "offset": 0, "length": 6 }],
                "reply_to_message": {
                    "message_id": 9, "date": 1, "chat": { "id": 7, "type": "private" }, "text": "hi"
                },
                "reply_markup": { "inline_keyboard": [[{ "text": "Go", "callback_data": "go" }]] }
            }),
            99,
        );
        assert_eq!((raw.id, raw.date, raw.message.as_str()), (10, 1_700_000_000, "/start now"));
        assert_eq!(raw.from_id, Some(get_peer(7)));
        assert_eq!(raw.peer_id, get_peer(7));
        assert!(!raw.out);
        let command = types::MessageEntityBotCommand { offset: 0, length: 6 };
        let entity = base::MessageEntity::MessageEntityBotCommand(command);
        assert_eq!(raw.entities, Some(vec![entity]));
        let Some(base::MessageReplyHeader::MessageReplyHeader(reply)) = &raw.reply_to else { panic!("no reply") };
        assert_eq!(reply.reply_to_msg_id, Some(9));
        let Some(base::ReplyMarkup::ReplyInlineMarkup(markup)) = &raw.reply_markup else { panic!("no keyboard") };
        let base::KeyboardInlineButtonRow::KeyboardInlineButtonRow(row) = &markup.rows[0];
        let base::KeyboardInlineButton::KeyboardInlineButton(button) = &row.buttons[0];
        let base::InlineButtonType::InlineButtonTypeCallback(data) = &button.r#type else { panic!("not a callback") };
        assert_eq!(data.data, b"go");
        // The sender and the private chat are one user, min and without an access hash.
        assert_eq!(peers.users.len(), 1);
        let base::User::User(user) = &peers.users[0] else { panic!("empty user") };
        assert!(user.min && user.access_hash.is_none());
        assert_eq!(user.username.as_deref(), Some("ann"));
    }

    #[test]
    fn channel_post_is_a_min_channel_with_no_sender() {
        let (raw, peers) = parse_message(
            json!({
                "message_id": 3,
                "date": 5,
                "chat": { "id": -1_001_234_567_890_i64, "type": "channel", "title": "News", "username": "news" },
                "sender_chat": { "id": -1_001_234_567_890_i64, "type": "channel", "title": "News" },
                "text": "hello"
            }),
            99,
        );
        assert!(raw.post);
        assert_eq!(raw.from_id, None);
        assert_eq!(raw.peer_id, base::Peer::PeerChannel(types::PeerChannel { channel_id: 1_234_567_890 }));
        let [base::Chat::Channel(channel)] = &peers.chats[..] else { panic!("expected one channel") };
        assert!(channel.min && channel.broadcast && !channel.megagroup && channel.access_hash.is_none());
        assert_eq!(channel.title, "News");
        assert!(matches!(message_update(raw, false), base::Update::UpdateNewChannelMessage(_)));
    }

    #[test]
    fn photo_and_document_decode_their_file_ids() {
        let photo_id = FileId {
            thumbnail_source: Some(ThumbnailSource::Thumbnail { file_type: crate::enums::FileType::Photo, size: 'y' }),
            ..FileId::new(crate::enums::FileType::Photo, 2, 555, 666, &[1, 2])
        }
        .encode();
        let document_id = FileId::new(crate::enums::FileType::Document, 4, 777, 888, &[3]).encode();
        let (raw, _) = parse_message(
            json!({
                "message_id": 1, "date": 5, "chat": { "id": -42, "type": "group", "title": "G" },
                "from": user_json(7),
                "photo": [{
                    "file_id": photo_id, "file_unique_id": "u", "width": 1280, "height": 720, "file_size": 99
                }],
                "caption": "look"
            }),
            99,
        );
        assert_eq!(raw.message, "look");
        let Some(base::MessageMedia::MessageMediaPhoto(media)) = &raw.media else { panic!("no photo") };
        let Some(base::Photo::Photo(photo)) = &media.photo else { panic!("empty photo") };
        assert_eq!((photo.id, photo.access_hash, photo.dc_id), (555, 666, 2));
        assert_eq!(photo.file_reference, vec![1, 2]);
        let Some(base::PhotoSize::PhotoSize(size)) = photo.sizes.first() else { panic!("no size") };
        assert_eq!((size.r#type.as_str(), size.w, size.h, size.size), ("y", 1280, 720, 99));

        let (raw, _) = parse_message(
            json!({
                "message_id": 2, "date": 5, "chat": { "id": -42, "type": "group", "title": "G" },
                "voice": { "file_id": document_id, "file_unique_id": "u", "duration": 3, "file_size": 10 }
            }),
            99,
        );
        let Some(base::MessageMedia::MessageMediaDocument(media)) = &raw.media else { panic!("no document") };
        assert!(media.voice);
        let Some(base::Document::Document(document)) = &media.document else { panic!("empty document") };
        assert_eq!((document.id, document.access_hash, document.dc_id, document.size), (777, 888, 4, 10));
        assert_eq!(document.mime_type, "audio/ogg");
    }

    #[test]
    fn callback_query_names_the_pressed_message() {
        let query: bot::CallbackQuery = serde_json::from_value(json!({
            "id": "-4242",
            "from": user_json(7),
            "message": {
                "message_id": 15, "date": 9, "chat": { "id": 7, "type": "private", "first_name": "Ann" }, "text": "pick"
            },
            "chat_instance": "123456789",
            "data": "go"
        }))
        .unwrap();
        let mut peers = Peers::new(99);
        let Some(base::Update::UpdateBotCallbackQuery(update)) = callback_query(query, &mut peers) else {
            panic!("not a callback query")
        };
        assert_eq!((update.query_id, update.user_id, update.msg_id), (-4242, 7, 15));
        assert_eq!((update.peer.clone(), update.chat_instance), (get_peer(7), 123_456_789));
        assert_eq!(update.data.as_deref(), Some(&b"go"[..]));
    }

    #[test]
    fn entities_go_back_out_as_bot_api_ones() {
        let text_url = types::MessageEntityTextUrl { offset: 1, length: 2, url: "https://e.x".into() };
        let mention = types::InputMessageEntityMentionName {
            offset: 3,
            length: 4,
            user_id: base::InputUser::InputUserSelf,
        };
        let list = vec![
            base::MessageEntity::MessageEntityTextUrl(Box::new(text_url)),
            base::MessageEntity::InputMessageEntityMentionName(Box::new(mention)),
            base::MessageEntity::MessageEntityBankCard(types::MessageEntityBankCard { offset: 0, length: 1 }),
        ];
        assert_eq!(
            entities_json(&list, 99),
            json!([
                { "type": "text_link", "offset": 1, "length": 2, "url": "https://e.x" },
                {
                    "type": "text_mention", "offset": 3, "length": 4,
                    "user": { "id": 99, "is_bot": false, "first_name": "" }
                }
            ])
        );
    }
}
