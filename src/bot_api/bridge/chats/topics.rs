//! Forum topics: creating, editing, closing, hiding and deleting them, and the custom emoji their icons can use.

use serde_json::{Map, Value, json};

use super::super::{Transport, unsupported, updates_bytes};
use super::{decode, no_updates, put};
use crate::bot_api::convert::{Peers, int};
use crate::bot_api::raw::types as bot;
use crate::errors::Result;
use crate::file_id::FileId;
use crate::raw::{Deserializable, Identifiable, Serializable, base, functions as f, types};
use crate::session::internals::now;
use crate::utils::{CHANNEL_OFFSET, get_peer};

const CREATE_TOPIC: u32 = <f::messages::CreateForumTopic as Identifiable>::CONSTRUCTOR_ID;
const EDIT_TOPIC: u32 = <f::messages::EditForumTopic as Identifiable>::CONSTRUCTOR_ID;
const DELETE_TOPIC: u32 = <f::messages::DeleteTopicHistory as Identifiable>::CONSTRUCTOR_ID;
const GET_STICKER_SET: u32 = <f::messages::GetStickerSet as Identifiable>::CONSTRUCTOR_ID;

/// The General topic of every forum has this fixed id; the Bot API edits it with methods of its own.
const GENERAL_TOPIC: i32 = 1;

impl Transport {
    /// `messages.getStickerSet` is this group's only for the topic icons; any other set is left to the next group.
    pub(super) async fn answer_chat_topics(&self, id: u32, body: &[u8]) -> Option<Result<Vec<u8>>> {
        Some(match id {
            CREATE_TOPIC => self.create_topic(body).await,
            EDIT_TOPIC => self.edit_topic(body).await,
            DELETE_TOPIC => self.delete_topic(body).await,
            GET_STICKER_SET => {
                let r = match f::messages::GetStickerSet::from_bytes(body) {
                    Ok(r) => r,
                    Err(e) => return Some(Err(e.into())),
                };
                if !matches!(r.stickerset, base::InputStickerSet::InputStickerSetEmojiDefaultTopicIcons) {
                    return None;
                }
                self.topic_icons().await
            }
            _ => return None,
        })
    }

    /// The Bot API answers the new topic, not the service message that created it, so the answer holds that
    /// message as TL tells of it, with the topic's id as its id.
    async fn create_topic(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::messages::CreateForumTopic::from_bytes(body)?;
        if r.send_as.is_some() || r.title_missing {
            return Err(unsupported("creating a topic as another chat or without a title"));
        }
        let me = self.me();
        let chat_id = self.chat_id(&r.peer)?;
        let mut body = Map::new();
        body.insert("chat_id".into(), chat_id.into());
        body.insert("name".into(), r.title.into());
        put(&mut body, "icon_color", r.icon_color);
        put(&mut body, "icon_custom_emoji_id", r.icon_emoji_id.map(|id| id.to_string()));
        let topic: bot::ForumTopic = decode(self.call("createForumTopic", Value::Object(body)).await?)?;
        let mut action = types::MessageActionTopicCreate::new(topic.name, int(topic.icon_color));
        action.icon_emoji_id = topic.icon_custom_emoji_id.and_then(|id| id.parse().ok());
        action.title_missing = topic.is_name_implicit.unwrap_or(false);
        let action = base::MessageAction::MessageActionTopicCreate(Box::new(action));
        let (id, date) = (int(topic.message_thread_id), int(now()));
        let mut created = types::MessageService::new(id, get_peer(chat_id), date, action);
        created.out = true;
        created.from_id = Some(get_peer(me));
        let message = base::Message::MessageService(Box::new(created));
        let update = if chat_id <= -CHANNEL_OFFSET {
            base::Update::UpdateNewChannelMessage(Box::new(types::UpdateNewChannelMessage {
                message,
                pts: 0,
                pts_count: 0,
            }))
        } else {
            base::Update::UpdateNewMessage(Box::new(types::UpdateNewMessage { message, pts: 0, pts_count: 0 }))
        };
        Ok(updates_bytes(vec![update], Peers::new(me)))
    }

    /// One Bot API method per change: the name and icon together, then closing, then hiding. The General topic has
    /// methods of its own, and no icon; only it can be hidden.
    async fn edit_topic(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::messages::EditForumTopic::from_bytes(body)?;
        let chat_id = self.chat_id(&r.peer)?;
        if r.topic_id == GENERAL_TOPIC {
            if r.icon_emoji_id.is_some() {
                return Err(unsupported("an icon for the General topic"));
            }
            if let Some(name) = r.title {
                self.call("editGeneralForumTopic", json!({ "chat_id": chat_id, "name": name })).await?;
            }
            if let Some(closed) = r.closed {
                let method = if closed { "closeGeneralForumTopic" } else { "reopenGeneralForumTopic" };
                self.call(method, json!({ "chat_id": chat_id })).await?;
            }
            if let Some(hidden) = r.hidden {
                let method = if hidden { "hideGeneralForumTopic" } else { "unhideGeneralForumTopic" };
                self.call(method, json!({ "chat_id": chat_id })).await?;
            }
            return Ok(no_updates(self.me()));
        }
        if r.hidden.is_some() {
            return Err(unsupported("hiding a topic other than General"));
        }
        if r.title.is_some() || r.icon_emoji_id.is_some() {
            let mut body = Map::new();
            body.insert("chat_id".into(), chat_id.into());
            body.insert("message_thread_id".into(), r.topic_id.into());
            put(&mut body, "name", r.title);
            // TL's 0 removes the icon, as the Bot API's empty string does.
            let icon = r.icon_emoji_id.map(|id| if id == 0 { String::new() } else { id.to_string() });
            put(&mut body, "icon_custom_emoji_id", icon);
            self.call("editForumTopic", Value::Object(body)).await?;
        }
        if let Some(closed) = r.closed {
            let method = if closed { "closeForumTopic" } else { "reopenForumTopic" };
            self.call(method, json!({ "chat_id": chat_id, "message_thread_id": r.topic_id })).await?;
        }
        Ok(no_updates(self.me()))
    }

    async fn delete_topic(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::messages::DeleteTopicHistory::from_bytes(body)?;
        let body = json!({ "chat_id": self.chat_id(&r.peer)?, "message_thread_id": r.top_msg_id });
        self.call("deleteForumTopic", body).await?;
        Ok(types::messages::AffectedHistory { pts: 0, pts_count: 0, offset: 0 }.to_bytes())
    }

    /// The topic icons as TL's emoji set. The Bot API names no set, so it has no id, title or short name.
    async fn topic_icons(&self) -> Result<Vec<u8>> {
        let stickers: Vec<bot::Sticker> = decode(self.call("getForumTopicIconStickers", json!({})).await?)?;
        let documents: Vec<_> = stickers.iter().filter_map(icon).collect();
        let count = i32::try_from(documents.len()).unwrap_or(i32::MAX);
        let mut set = types::StickerSet::new(0, 0, String::new(), String::new(), count, 0);
        set.emojis = true;
        let set = base::StickerSet::StickerSet(Box::new(set));
        Ok(types::messages::StickerSet { set, packs: Vec::new(), keywords: Vec::new(), documents }.to_bytes())
    }

    /// `unpin_all_general_forum_topic_messages`: the Bot API unpins the General topic with a method of its own.
    pub(crate) async fn unpin_all_general_forum_topic_messages(&self, chat_id: i64) -> Result<()> {
        self.call("unpinAllGeneralForumTopicMessages", json!({ "chat_id": chat_id })).await?;
        Ok(())
    }
}

/// A topic icon as the TL document of a custom emoji, its id the emoji's; `None` when its file id can't be read.
fn icon(sticker: &bot::Sticker) -> Option<base::Document> {
    let file = FileId::decode(&sticker.file_id).ok()?;
    let id = sticker.custom_emoji_id.as_deref().and_then(|id| id.parse().ok()).unwrap_or(file.media_id);
    let mime_type = if sticker.is_video {
        "video/webm"
    } else if sticker.is_animated {
        "application/x-tgsticker"
    } else {
        "image/webp"
    };
    let alt = sticker.emoji.clone().unwrap_or_default();
    let set = base::InputStickerSet::InputStickerSetEmojiDefaultTopicIcons;
    let emoji = types::DocumentAttributeCustomEmoji::new(alt, set);
    let attributes = vec![base::DocumentAttribute::DocumentAttributeCustomEmoji(Box::new(emoji))];
    let document = types::Document::new(
        id,
        file.access_hash,
        file.file_reference,
        0,
        mime_type.into(),
        sticker.file_size.unwrap_or(0),
        file.dc_id,
        attributes,
    );
    Some(base::Document::Document(Box::new(document)))
}
