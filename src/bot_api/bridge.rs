//! The bridge of a client in Bot API mode ([`ClientBuilder::bot_api`](crate::ClientBuilder::bot_api)): the session
//! hands it each TL request a friendly method makes, it sends the Bot API method that does the same and answers with
//! the TL the friendly method reads back. Answers are partly made up: see [`super::convert`].

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use serde::Deserialize;
use serde_json::{Map, Value, json};
use tokio::sync::mpsc;

use super::BotApi;
use super::client::invalid;
use super::convert::{self, Peers, int};
use super::raw::types as bot;
use crate::errors::{Error, Result};
use crate::raw::{Deserializable, Identifiable, Serializable, base, functions as f, types};
use crate::session::internals::now;
use crate::session::{Incoming, Responder};
use crate::utils::{CHANNEL_OFFSET, channel_id_of, get_channel_id, get_peer, get_peer_id, input_peer_id};

/// How many message ids of private chats and basic groups the bridge remembers the chat of, for deletes.
const SEEN_CHATS: usize = 4096;

const WITHOUT_UPDATES: u32 = <f::InvokeWithoutUpdates<()> as Identifiable>::CONSTRUCTOR_ID;
const GET_USERS: u32 = <f::users::GetUsers as Identifiable>::CONSTRUCTOR_ID;
const GET_FULL_USER: u32 = <f::users::GetFullUser as Identifiable>::CONSTRUCTOR_ID;
const GET_STATE: u32 = <f::updates::GetState as Identifiable>::CONSTRUCTOR_ID;
const GET_DIFFERENCE: u32 = <f::updates::GetDifference as Identifiable>::CONSTRUCTOR_ID;
const SEND_MESSAGE: u32 = <f::messages::SendMessage as Identifiable>::CONSTRUCTOR_ID;
const EDIT_MESSAGE: u32 = <f::messages::EditMessage as Identifiable>::CONSTRUCTOR_ID;
const DELETE_MESSAGES: u32 = <f::messages::DeleteMessages as Identifiable>::CONSTRUCTOR_ID;
const DELETE_CHANNEL_MESSAGES: u32 = <f::channels::DeleteMessages as Identifiable>::CONSTRUCTOR_ID;
const FORWARD_MESSAGES: u32 = <f::messages::ForwardMessages as Identifiable>::CONSTRUCTOR_ID;
const SET_CALLBACK_ANSWER: u32 = <f::messages::SetBotCallbackAnswer as Identifiable>::CONSTRUCTOR_ID;
const SET_INLINE_RESULTS: u32 = <f::messages::SetInlineBotResults as Identifiable>::CONSTRUCTOR_ID;
const SET_TYPING: u32 = <f::messages::SetTyping as Identifiable>::CONSTRUCTOR_ID;
const GET_CHATS: u32 = <f::messages::GetChats as Identifiable>::CONSTRUCTOR_ID;
const GET_CHANNELS: u32 = <f::channels::GetChannels as Identifiable>::CONSTRUCTOR_ID;
const GET_FULL_CHAT: u32 = <f::messages::GetFullChat as Identifiable>::CONSTRUCTOR_ID;
const GET_FULL_CHANNEL: u32 = <f::channels::GetFullChannel as Identifiable>::CONSTRUCTOR_ID;
const UPDATE_PINNED: u32 = <f::messages::UpdatePinnedMessage as Identifiable>::CONSTRUCTOR_ID;
const UNPIN_ALL: u32 = <f::messages::UnpinAllMessages as Identifiable>::CONSTRUCTOR_ID;
const SET_COMMANDS: u32 = <f::bots::SetBotCommands as Identifiable>::CONSTRUCTOR_ID;

/// A request a client in Bot API mode can't make: its TL method, or a part of it, has no Bot API counterpart in
/// vaegram yet. It comes inside `Error::Other`; [`Error::downcast_ref`] reads it back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unsupported(&'static str);

impl Unsupported {
    /// The TL method, or the part of a request, with no counterpart.
    pub fn what(&self) -> &'static str {
        self.0
    }
}

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} isn't available over the Bot API yet", self.0)
    }
}

impl std::error::Error for Unsupported {}

pub(crate) fn unsupported(what: &'static str) -> Error {
    Error::other(Unsupported(what))
}

/// What a client in Bot API mode talks through: the HTTP client, the queue `getUpdates` feeds, and what the bridge
/// keeps between requests.
pub(crate) struct Transport {
    pub(crate) api: BotApi,
    pub(crate) updates: mpsc::Sender<Incoming>,
    /// The next `getUpdates` offset: one past the last update taken.
    pub(crate) offset: AtomicI64,
    me: AtomicI64,
    chats: Mutex<SeenChats>,
}

/// TL's `messages.deleteMessages` names no chat: ids outside channels are unique per account, and the Bot API wants
/// the chat, so the bridge remembers it for the messages it saw, oldest forgotten first.
#[derive(Default)]
struct SeenChats {
    chats: HashMap<i32, i64>,
    order: VecDeque<i32>,
}

impl Transport {
    pub(crate) fn new(api: BotApi, updates: mpsc::Sender<Incoming>) -> Transport {
        Transport { api, updates, offset: AtomicI64::new(0), me: AtomicI64::new(0), chats: Mutex::default() }
    }

    /// The bot's own id, 0 until `getMe` answered.
    pub(crate) fn me(&self) -> i64 {
        self.me.load(Ordering::Relaxed)
    }

    /// Remembers the chat of a message outside channels, so a delete can name it.
    pub(crate) fn saw(&self, message: &types::Message) {
        let chat = get_peer_id(&message.peer_id);
        if chat <= -CHANNEL_OFFSET {
            return;
        }
        let mut seen = self.chats.lock().unwrap_or_else(PoisonError::into_inner);
        if seen.chats.insert(message.id, chat).is_none() {
            seen.order.push_back(message.id);
            if seen.order.len() > SEEN_CHATS {
                if let Some(old) = seen.order.pop_front() {
                    seen.chats.remove(&old);
                }
            }
        }
    }

    fn chat_of(&self, id: i32) -> Option<i64> {
        self.chats.lock().unwrap_or_else(PoisonError::into_inner).chats.get(&id).copied()
    }

    async fn call(&self, method: &str, body: Value) -> Result<Value> {
        self.api.call(method, &body).await
    }

    fn chat_id(&self, peer: &base::InputPeer) -> Result<i64> {
        input_peer_id(peer, self.me()).ok_or(Error::InvalidArgument("the request names no chat"))
    }

    /// Answers one TL request with the bytes of its TL answer.
    async fn answer(&self, name: &'static str, body: &[u8]) -> Result<Vec<u8>> {
        let mut body = body;
        // What a client sends while updates are off comes wrapped; the bridge answers the call inside.
        while constructor(body) == Some(WITHOUT_UPDATES) {
            body = body.get(4..).unwrap_or_default();
        }
        match constructor(body) {
            Some(GET_USERS) => self.get_users(f::users::GetUsers::from_bytes(body)?).await,
            Some(GET_FULL_USER) => {
                let r = f::users::GetFullUser::from_bytes(body)?;
                let id = convert::input_user_id(&r.id, self.me()).ok_or(Error::InvalidArgument("no such user"))?;
                self.get_full(id).await
            }
            Some(GET_STATE) => {
                Ok(types::updates::State { pts: 0, qts: 0, date: int(now()), seq: 0, unread_count: 0 }.to_bytes())
            }
            // Updates come from `getUpdates`, unnumbered, so there is never a gap to fill.
            Some(GET_DIFFERENCE) => Ok(types::updates::DifferenceEmpty { date: int(now()), seq: 0 }.to_bytes()),
            Some(SEND_MESSAGE) => self.send_message(f::messages::SendMessage::from_bytes(body)?).await,
            Some(EDIT_MESSAGE) => self.edit_message(f::messages::EditMessage::from_bytes(body)?).await,
            Some(DELETE_MESSAGES) => {
                self.delete_messages(f::messages::DeleteMessages::from_bytes(body)?.id, None).await
            }
            Some(DELETE_CHANNEL_MESSAGES) => {
                let r = f::channels::DeleteMessages::from_bytes(body)?;
                self.delete_messages(r.id, Some(channel_chat_id(&r.channel)?)).await
            }
            Some(FORWARD_MESSAGES) => self.forward_messages(f::messages::ForwardMessages::from_bytes(body)?).await,
            Some(SET_CALLBACK_ANSWER) => {
                self.answer_callback_query(f::messages::SetBotCallbackAnswer::from_bytes(body)?).await
            }
            Some(SET_INLINE_RESULTS) => {
                self.answer_inline_query(f::messages::SetInlineBotResults::from_bytes(body)?).await
            }
            Some(SET_TYPING) => self.send_chat_action(f::messages::SetTyping::from_bytes(body)?).await,
            Some(GET_CHATS) => {
                let ids = f::messages::GetChats::from_bytes(body)?.id.into_iter().map(i64::wrapping_neg).collect();
                self.get_chats(ids).await
            }
            Some(GET_CHANNELS) => {
                let r = f::channels::GetChannels::from_bytes(body)?;
                self.get_chats(r.id.iter().filter_map(|c| channel_chat_id(c).ok()).collect()).await
            }
            Some(GET_FULL_CHAT) => {
                self.get_full(f::messages::GetFullChat::from_bytes(body)?.chat_id.wrapping_neg()).await
            }
            Some(GET_FULL_CHANNEL) => {
                self.get_full(channel_chat_id(&f::channels::GetFullChannel::from_bytes(body)?.channel)?).await
            }
            Some(UPDATE_PINNED) => self.pin(f::messages::UpdatePinnedMessage::from_bytes(body)?).await,
            Some(UNPIN_ALL) => self.unpin_all(f::messages::UnpinAllMessages::from_bytes(body)?).await,
            Some(SET_COMMANDS) => self.set_commands(f::bots::SetBotCommands::from_bytes(body)?).await,
            _ => Err(unsupported(name)),
        }
    }

    /// `InputUserSelf` is `getMe`; any other user is looked up with `getChat`, and left out when the bot can't see
    /// it, as Telegram leaves out a user it doesn't know.
    async fn get_users(&self, r: f::users::GetUsers) -> Result<Vec<u8>> {
        let mut users = Vec::with_capacity(r.id.len());
        for user in &r.id {
            if matches!(user, base::InputUser::InputUserSelf) {
                users.push(self.get_me().await?);
                continue;
            }
            let Some(id) = convert::input_user_id(user, self.me()) else { continue };
            if let Some(chat) = self.find_chat(id).await? {
                users.push(convert::chat_user(&chat, true));
            }
        }
        Ok(users.to_bytes())
    }

    /// The bot's own user. `getMe` answers more than its documented `User` in some Bot API versions, so only the
    /// fields used here are read.
    async fn get_me(&self) -> Result<base::User> {
        #[derive(Deserialize)]
        struct Me {
            id: i64,
            #[serde(default)]
            is_bot: bool,
            #[serde(default)]
            first_name: String,
            last_name: Option<String>,
            username: Option<String>,
        }
        let me = Me::deserialize(&self.call("getMe", json!({})).await?).map_err(invalid)?;
        self.me.store(me.id, Ordering::Relaxed);
        let mut user = types::User::new(me.id);
        user.is_self = true;
        user.bot = me.is_bot;
        user.access_hash = Some(0);
        user.first_name = Some(me.first_name);
        user.last_name = me.last_name;
        user.username = me.username;
        Ok(base::User::User(Box::new(user)))
    }

    /// `getChat`, with `None` for a chat the bot can't see.
    async fn find_chat(&self, chat_id: i64) -> Result<Option<bot::Chat>> {
        match self.call("getChat", json!({ "chat_id": chat_id })).await {
            Ok(value) => Ok(Some(bot::Chat::deserialize(&value).map_err(invalid)?)),
            Err(Error::Rpc(e)) if e.code == 400 || e.code == 403 => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn get_chats(&self, ids: Vec<i64>) -> Result<Vec<u8>> {
        let mut chats = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(chat) = self.find_chat(id).await?.as_ref().and_then(|c| convert::chat(c, true)) {
                chats.push(chat);
            }
        }
        Ok(types::messages::Chats { chats }.to_bytes())
    }

    /// The full user or chat `getChat` describes: its bio or description, pinned message and slow mode. The rest
    /// of the TL full types is left empty.
    async fn get_full(&self, chat_id: i64) -> Result<Vec<u8>> {
        #[derive(Deserialize)]
        struct Extra {
            bio: Option<String>,
            description: Option<String>,
            pinned_message: Option<bot::MessageId>,
            slow_mode_delay: Option<i64>,
        }
        let value = self.call("getChat", json!({ "chat_id": chat_id })).await?;
        let chat = bot::Chat::deserialize(&value).map_err(invalid)?;
        let extra = Extra::deserialize(&value).map_err(invalid)?;
        let pinned = extra.pinned_message.map(|m| int(m.message_id));
        let notify = base::PeerNotifySettings::PeerNotifySettings(Box::default());
        if chat_id > 0 {
            let settings = base::PeerSettings::PeerSettings(Box::default());
            let mut full = types::UserFull::new(chat_id, settings, notify, 0);
            full.about = extra.bio;
            full.pinned_msg_id = pinned;
            let full_user = base::UserFull::UserFull(Box::new(full));
            let users = vec![convert::chat_user(&chat, true)];
            return Ok(types::users::UserFull { full_user, chats: Vec::new(), users }.to_bytes());
        }
        let about = extra.description.unwrap_or_default();
        let full_chat = if chat_id > -CHANNEL_OFFSET {
            let id = chat_id.wrapping_neg();
            let forbidden = types::ChatParticipantsForbidden { chat_id: id, self_participant: None };
            let participants = base::ChatParticipants::ChatParticipantsForbidden(Box::new(forbidden));
            let mut full = types::ChatFull::new(id, about, participants, notify);
            full.pinned_msg_id = pinned;
            base::ChatFull::ChatFull(Box::new(full))
        } else {
            let photo = base::Photo::PhotoEmpty(types::PhotoEmpty { id: 0 });
            let id = channel_id_of(chat_id);
            let mut full = types::ChannelFull::new(id, about, 0, 0, 0, photo, notify, Vec::new(), 0);
            full.pinned_msg_id = pinned;
            full.slowmode_seconds = extra.slow_mode_delay.map(int);
            base::ChatFull::ChannelFull(Box::new(full))
        };
        let chats = convert::chat(&chat, true).into_iter().collect();
        Ok(types::messages::ChatFull { full_chat, chats, users: Vec::new() }.to_bytes())
    }

    async fn send_message(&self, r: f::messages::SendMessage) -> Result<Vec<u8>> {
        if r.schedule_date.is_some() {
            return Err(unsupported("scheduled messages"));
        }
        if r.send_as.is_some() || r.quick_reply_shortcut.is_some() || r.rich_message.is_some() {
            return Err(unsupported("sending as another chat, quick replies or rich messages"));
        }
        let me = self.me();
        let mut body = Map::new();
        body.insert("chat_id".into(), self.chat_id(&r.peer)?.into());
        body.insert("text".into(), r.message.into());
        if let Some(list) = &r.entities {
            body.insert("entities".into(), convert::entities_json(list, me));
        }
        if r.no_webpage || r.invert_media {
            let preview = json!({ "is_disabled": r.no_webpage, "show_above_text": r.invert_media });
            body.insert("link_preview_options".into(), preview);
        }
        options(&mut body, r.silent, r.noforwards, r.allow_paid_floodskip);
        if let Some(reply) = &r.reply_to {
            reply_parameters(&mut body, reply, me)?;
        }
        if let Some(markup) = &r.reply_markup {
            body.insert("reply_markup".into(), convert::markup_json(markup, me)?);
        }
        if let Some(effect) = r.effect {
            body.insert("message_effect_id".into(), effect.to_string().into());
        }
        let sent = self.call("sendMessage", Value::Object(body)).await?;
        self.sent(vec![sent], false)
    }

    /// No text and no media is `editMessageReplyMarkup`; text is `editMessageText`, or `editMessageCaption` when
    /// the message turns out to be a media one, as the Bot API keeps the two apart.
    async fn edit_message(&self, r: f::messages::EditMessage) -> Result<Vec<u8>> {
        if r.schedule_date.is_some() || r.quick_reply_shortcut_id.is_some() || r.rich_message.is_some() {
            return Err(unsupported("editing scheduled, quick reply or rich messages"));
        }
        let me = self.me();
        let mut body = Map::new();
        body.insert("chat_id".into(), self.chat_id(&r.peer)?.into());
        body.insert("message_id".into(), r.id.into());
        if let Some(markup) = &r.reply_markup {
            body.insert("reply_markup".into(), convert::markup_json(markup, me)?);
        }
        let page = match &r.media {
            None => None,
            Some(base::InputMedia::InputMediaWebPage(page)) => Some(page),
            Some(_) => return Err(unsupported("editing a message's media")),
        };
        let Some(text) = r.message else {
            let edited = self.call("editMessageReplyMarkup", Value::Object(body)).await?;
            return self.sent(vec![edited], true);
        };
        let mut caption = body.clone();
        body.insert("text".into(), text.clone().into());
        if let Some(list) = &r.entities {
            body.insert("entities".into(), convert::entities_json(list, me));
        }
        let mut preview = Map::new();
        if r.no_webpage {
            preview.insert("is_disabled".into(), true.into());
        }
        if let Some(page) = page {
            preview.insert("url".into(), page.url.clone().into());
            preview.insert("prefer_large_media".into(), page.force_large_media.into());
            preview.insert("prefer_small_media".into(), page.force_small_media.into());
        }
        if r.invert_media {
            preview.insert("show_above_text".into(), true.into());
        }
        if !preview.is_empty() {
            body.insert("link_preview_options".into(), Value::Object(preview));
        }
        match self.call("editMessageText", Value::Object(body)).await {
            Ok(edited) => self.sent(vec![edited], true),
            Err(Error::Rpc(e)) if e.name == "THERE_IS_NO_TEXT_IN_THE_MESSAGE_TO_EDIT" => {
                caption.insert("caption".into(), text.into());
                if let Some(list) = &r.entities {
                    caption.insert("caption_entities".into(), convert::entities_json(list, me));
                }
                if r.invert_media {
                    caption.insert("show_caption_above_media".into(), true.into());
                }
                let edited = self.call("editMessageCaption", Value::Object(caption)).await?;
                self.sent(vec![edited], true)
            }
            Err(e) => Err(e),
        }
    }

    /// `chat` for a channel's messages; other ids name their chat only through the messages the bridge saw.
    async fn delete_messages(&self, ids: Vec<i32>, chat: Option<i64>) -> Result<Vec<u8>> {
        let mut by_chat: Vec<(i64, Vec<i32>)> = Vec::new();
        for id in ids {
            let chat = match chat.or_else(|| self.chat_of(id)) {
                Some(chat) => chat,
                None => {
                    return Err(Error::InvalidArgument(
                        "over the Bot API, only messages this client saw can be deleted outside channels",
                    ));
                }
            };
            match by_chat.iter_mut().find(|(known, _)| *known == chat) {
                Some((_, ids)) => ids.push(id),
                None => by_chat.push((chat, vec![id])),
            }
        }
        for (chat_id, message_ids) in by_chat {
            self.call("deleteMessages", json!({ "chat_id": chat_id, "message_ids": message_ids })).await?;
        }
        Ok(types::messages::AffectedMessages { pts: 0, pts_count: 0 }.to_bytes())
    }

    /// One `forwardMessage` per message, or `copyMessage` when the author is dropped. A copy comes back as its id
    /// only, so its TL message holds no more than that.
    async fn forward_messages(&self, r: f::messages::ForwardMessages) -> Result<Vec<u8>> {
        if r.schedule_date.is_some() || r.send_as.is_some() || r.quick_reply_shortcut.is_some() {
            return Err(unsupported("scheduled forwards, forwards as another chat or to quick replies"));
        }
        if r.reply_to.is_some() && !r.drop_author {
            return Err(unsupported("forwards that reply to a message"));
        }
        let me = self.me();
        let to = self.chat_id(&r.to_peer)?;
        let mut common = Map::new();
        common.insert("chat_id".into(), to.into());
        common.insert("from_chat_id".into(), self.chat_id(&r.from_peer)?.into());
        options(&mut common, r.silent, r.noforwards, r.allow_paid_floodskip);
        if let Some(top) = r.top_msg_id {
            common.insert("message_thread_id".into(), top.into());
        }
        if let Some(reply) = &r.reply_to {
            reply_parameters(&mut common, reply, me)?;
        }
        if let Some(start) = r.video_timestamp {
            common.insert("video_start_timestamp".into(), start.into());
        }
        let mut peers = Peers::new(me);
        let mut updates = Vec::with_capacity(r.id.len());
        for &id in &r.id {
            let mut body = common.clone();
            body.insert("message_id".into(), id.into());
            let raw = if r.drop_author {
                if r.drop_media_captions {
                    body.insert("remove_caption".into(), true.into());
                }
                let copied = self.call("copyMessage", Value::Object(body)).await?;
                let copied = bot::MessageId::deserialize(&copied).map_err(invalid)?;
                let mut raw = types::Message::new(int(copied.message_id), get_peer(to), int(now()), String::new());
                raw.out = true;
                raw
            } else {
                let forwarded = self.call("forwardMessage", Value::Object(body)).await?;
                convert::message(bot::Message::deserialize(&forwarded).map_err(invalid)?, &mut peers)
            };
            self.saw(&raw);
            updates.push(convert::message_update(raw, false));
        }
        Ok(updates_bytes(updates, peers))
    }

    async fn answer_callback_query(&self, r: f::messages::SetBotCallbackAnswer) -> Result<Vec<u8>> {
        let mut body = Map::new();
        body.insert("callback_query_id".into(), r.query_id.to_string().into());
        if let Some(text) = r.message {
            body.insert("text".into(), text.into());
        }
        if r.alert {
            body.insert("show_alert".into(), true.into());
        }
        if let Some(url) = r.url {
            body.insert("url".into(), url.into());
        }
        if r.cache_time != 0 {
            body.insert("cache_time".into(), r.cache_time.into());
        }
        self.call("answerCallbackQuery", Value::Object(body)).await?;
        Ok(true.to_bytes())
    }

    async fn answer_inline_query(&self, r: f::messages::SetInlineBotResults) -> Result<Vec<u8>> {
        let me = self.me();
        let results = r.results.iter().map(|result| convert::inline_result_json(result, me)).collect::<Result<_>>()?;
        let mut body = Map::new();
        body.insert("inline_query_id".into(), r.query_id.to_string().into());
        body.insert("results".into(), Value::Array(results));
        body.insert("cache_time".into(), r.cache_time.into());
        if r.private {
            body.insert("is_personal".into(), true.into());
        }
        if let Some(offset) = r.next_offset {
            body.insert("next_offset".into(), offset.into());
        }
        let button = match (r.switch_pm, r.switch_webview) {
            (Some(base::InlineBotSwitchPM::InlineBotSwitchPM(pm)), _) => {
                Some(json!({ "text": pm.text, "start_parameter": pm.start_param }))
            }
            (None, Some(base::InlineBotWebView::InlineBotWebView(view))) => {
                Some(json!({ "text": view.text, "web_app": { "url": view.url } }))
            }
            _ => None,
        };
        if let Some(button) = button {
            body.insert("button".into(), button);
        }
        self.call("answerInlineQuery", Value::Object(body)).await?;
        Ok(true.to_bytes())
    }

    async fn send_chat_action(&self, r: f::messages::SetTyping) -> Result<Vec<u8>> {
        use base::SendMessageAction as A;
        let action = match r.action {
            // The Bot API has no cancel: an action ends by itself after 5 seconds or with the next message.
            A::SendMessageCancelAction => return Ok(true.to_bytes()),
            A::SendMessageTypingAction => "typing",
            A::SendMessageRecordVideoAction => "record_video",
            A::SendMessageUploadVideoAction(_) => "upload_video",
            A::SendMessageRecordAudioAction => "record_voice",
            A::SendMessageUploadAudioAction(_) => "upload_voice",
            A::SendMessageUploadPhotoAction(_) => "upload_photo",
            A::SendMessageUploadDocumentAction(_) => "upload_document",
            A::SendMessageGeoLocationAction => "find_location",
            A::SendMessageRecordRoundAction => "record_video_note",
            A::SendMessageUploadRoundAction(_) => "upload_video_note",
            A::SendMessageChooseStickerAction => "choose_sticker",
            _ => return Err(unsupported("this chat action")),
        };
        let mut body = Map::new();
        body.insert("chat_id".into(), self.chat_id(&r.peer)?.into());
        body.insert("action".into(), action.into());
        if let Some(top) = r.top_msg_id {
            body.insert("message_thread_id".into(), top.into());
        }
        self.call("sendChatAction", Value::Object(body)).await?;
        Ok(true.to_bytes())
    }

    /// The service message a pin makes isn't returned by the Bot API, so the answer holds no update.
    async fn pin(&self, r: f::messages::UpdatePinnedMessage) -> Result<Vec<u8>> {
        let chat_id = self.chat_id(&r.peer)?;
        if r.unpin {
            self.call("unpinChatMessage", json!({ "chat_id": chat_id, "message_id": r.id })).await?;
        } else {
            let body = json!({ "chat_id": chat_id, "message_id": r.id, "disable_notification": r.silent });
            self.call("pinChatMessage", body).await?;
        }
        Ok(updates_bytes(Vec::new(), Peers::new(self.me())))
    }

    async fn unpin_all(&self, r: f::messages::UnpinAllMessages) -> Result<Vec<u8>> {
        if r.saved_peer_id.is_some() {
            return Err(unsupported("unpinning in saved messages"));
        }
        let chat_id = self.chat_id(&r.peer)?;
        match r.top_msg_id {
            Some(topic) => {
                let body = json!({ "chat_id": chat_id, "message_thread_id": topic });
                self.call("unpinAllForumTopicMessages", body).await?
            }
            None => self.call("unpinAllChatMessages", json!({ "chat_id": chat_id })).await?,
        };
        Ok(types::messages::AffectedHistory { pts: 0, pts_count: 0, offset: 0 }.to_bytes())
    }

    async fn set_commands(&self, r: f::bots::SetBotCommands) -> Result<Vec<u8>> {
        use base::BotCommandScope as S;
        let me = self.me();
        let scope = match &r.scope {
            S::BotCommandScopeDefault => json!({ "type": "default" }),
            S::BotCommandScopeUsers => json!({ "type": "all_private_chats" }),
            S::BotCommandScopeChats => json!({ "type": "all_group_chats" }),
            S::BotCommandScopeChatAdmins => json!({ "type": "all_chat_administrators" }),
            S::BotCommandScopePeer(s) => json!({ "type": "chat", "chat_id": self.chat_id(&s.peer)? }),
            S::BotCommandScopePeerAdmins(s) => {
                json!({ "type": "chat_administrators", "chat_id": self.chat_id(&s.peer)? })
            }
            S::BotCommandScopePeerUser(s) => {
                let chat_id = self.chat_id(&s.peer)?;
                let user_id = convert::input_user_id(&s.user_id, me).ok_or(Error::InvalidArgument("no such user"))?;
                json!({ "type": "chat_member", "chat_id": chat_id, "user_id": user_id })
            }
        };
        let commands: Vec<Value> = r
            .commands
            .iter()
            .map(|base::BotCommand::BotCommand(c)| json!({ "command": c.command, "description": c.description }))
            .collect();
        let mut body = Map::new();
        body.insert("commands".into(), commands.into());
        body.insert("scope".into(), scope);
        if !r.lang_code.is_empty() {
            body.insert("language_code".into(), r.lang_code.into());
        }
        self.call("setMyCommands", Value::Object(body)).await?;
        Ok(true.to_bytes())
    }

    /// The TL answer to a send or an edit: the messages the Bot API returned, as `Updates`.
    fn sent(&self, messages: Vec<Value>, edit: bool) -> Result<Vec<u8>> {
        let mut peers = Peers::new(self.me());
        let mut updates = Vec::with_capacity(messages.len());
        for value in messages {
            let raw = convert::message(bot::Message::deserialize(&value).map_err(invalid)?, &mut peers);
            self.saw(&raw);
            updates.push(convert::message_update(raw, edit));
        }
        Ok(updates_bytes(updates, peers))
    }
}

/// What the session calls with each request of a client in Bot API mode.
pub(crate) fn responder(transport: Arc<Transport>) -> Responder {
    Arc::new(move |name: &'static str, body: Vec<u8>| -> crate::session::Reply {
        let transport = transport.clone();
        Box::pin(async move { transport.answer(name, &body).await })
    })
}

fn constructor(body: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(body.get(..4)?.try_into().ok()?))
}

fn channel_chat_id(channel: &base::InputChannel) -> Result<i64> {
    match channel {
        base::InputChannel::InputChannel(c) => Ok(get_channel_id(c.channel_id)),
        base::InputChannel::InputChannelFromMessage(c) => Ok(get_channel_id(c.channel_id)),
        base::InputChannel::InputChannelEmpty => Err(Error::InvalidArgument("the request names no channel")),
    }
}

fn updates_bytes(updates: Vec<base::Update>, peers: Peers) -> Vec<u8> {
    types::Updates { updates, users: peers.users, chats: peers.chats, date: int(now()), seq: 0 }.to_bytes()
}

/// The send options TL and the Bot API share.
fn options(body: &mut Map<String, Value>, silent: bool, protect: bool, paid: bool) {
    let flags = [("disable_notification", silent), ("protect_content", protect), ("allow_paid_broadcast", paid)];
    for (key, _) in flags.into_iter().filter(|&(_, on)| on) {
        body.insert(key.into(), true.into());
    }
}

fn reply_parameters(body: &mut Map<String, Value>, reply: &base::InputReplyTo, me: i64) -> Result<()> {
    let base::InputReplyTo::InputReplyToMessage(r) = reply else {
        return Err(unsupported("replies to stories or in direct messages topics"));
    };
    let mut parameters = Map::new();
    parameters.insert("message_id".into(), r.reply_to_msg_id.into());
    if let Some(chat_id) = r.reply_to_peer_id.as_ref().and_then(|peer| input_peer_id(peer, me)) {
        parameters.insert("chat_id".into(), chat_id.into());
    }
    if let Some(quote) = &r.quote_text {
        parameters.insert("quote".into(), quote.clone().into());
    }
    if let Some(list) = &r.quote_entities {
        parameters.insert("quote_entities".into(), convert::entities_json(list, me));
    }
    if let Some(offset) = r.quote_offset {
        parameters.insert("quote_position".into(), offset.into());
    }
    if let Some(task) = r.todo_item_id {
        parameters.insert("checklist_task_id".into(), task.into());
    }
    body.insert("reply_parameters".into(), Value::Object(parameters));
    if let Some(top) = r.top_msg_id {
        body.insert("message_thread_id".into(), top.into());
    }
    Ok(())
}
