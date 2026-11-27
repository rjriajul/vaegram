//! Friendly methods' own branches in Bot API mode: profile photos and business stories, whose files go in the
//! request of the Bot API method that uses them, and sticker set changes, which name a sticker by its Bot API file
//! id. TL's upload flow never starts.

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::parse;
use crate::Client;
use crate::bot_api::bridge::{Transport, unsupported};
use crate::bot_api::convert::{self, Peers, int};
use crate::bot_api::input_file::{Attachment, marker};
use crate::bot_api::raw::types as bot;
use crate::dispatcher::Entities;
use crate::enums::{FileType, MaskPointType, StickerType};
use crate::errors::{Error, Result};
use crate::file_id::FileId;
use crate::raw::{base, types};
use crate::session::internals::now;
use crate::types::messages_and_media::StickerFile;
use crate::types::{
    FileLocation, InputSticker, InputStoryContent, MaskPosition, PeerRef, ReactionType, Sticker, Story, StoryArea,
    StoryAreaType, Upload,
};
use crate::utils::get_peer;

/// What a business story shows, its caption and caption entities written out, and its areas.
type StoryParts<'a> = (InputStoryContent<'a>, String, Vec<base::MessageEntity>, &'a [StoryArea]);

impl Transport {
    /// The id a request names a chat or user by, with no lookup for an id.
    pub(crate) async fn peer_id(&self, client: &Client, peer: &PeerRef) -> Result<i64> {
        match peer {
            PeerRef::Id(id) => Ok(*id),
            other => Ok(client.resolve(other).await?.0),
        }
    }

    /// `set_profile_photo` and `set_business_account_profile_photo`: `setMyProfilePhoto`, or
    /// `setBusinessAccountProfilePhoto` through `connection`, with the file in the request.
    pub(crate) async fn set_profile_photo(
        &self,
        connection: Option<&str>,
        file: Upload<'_>,
        video: bool,
        video_start_ts: Option<f64>,
        public: bool,
    ) -> Result<()> {
        let mut files = Vec::new();
        let photo = if video {
            let mut photo = Map::new();
            photo.insert("type".into(), "animated".into());
            photo.insert("animation".into(), attach(&mut files, file).await?);
            if let Some(seconds) = video_start_ts {
                photo.insert("main_frame_timestamp".into(), seconds.into());
            }
            Value::Object(photo)
        } else {
            json!({ "type": "static", "photo": attach(&mut files, file).await? })
        };
        let (method, body) = match connection {
            Some(connection) => {
                let body = json!({ "business_connection_id": connection, "photo": photo, "is_public": public });
                ("setBusinessAccountProfilePhoto", body)
            }
            None if public => return Err(unsupported("the public profile photo of a bot")),
            None => ("setMyProfilePhoto", json!({ "photo": photo })),
        };
        self.api.call_with_files::<Value>(method, body, files).await?;
        Ok(())
    }

    /// `post_business_story`: `postStory` with the story's file. The Bot API answers with the story's chat and id
    /// alone, so the story returned holds those, what the request set, and no media.
    pub(crate) async fn post_story(
        &self,
        client: &Client,
        connection: &str,
        story: StoryParts<'_>,
        active_period: i32,
        (post_to_chat_page, protect_content): (bool, bool),
    ) -> Result<Story> {
        let (content, caption, entities, areas) = story;
        let mut files = Vec::new();
        let text = (caption.as_str(), entities.as_slice());
        let mut body = story_body(&mut files, connection, content, text, areas, self.me()).await?;
        body.insert("active_period".into(), active_period.into());
        flag(&mut body, "post_to_chat_page", post_to_chat_page);
        flag(&mut body, "protect_content", protect_content);
        let posted = self.api.call_with_files::<Value>("postStory", Value::Object(body), files).await?;
        let mut posted = self.story(client, posted)?;
        posted.expire_date = posted.date.saturating_add(active_period);
        posted.caption = Some(caption).filter(|caption| !caption.is_empty());
        (posted.is_pinned, posted.has_protected_content) = (post_to_chat_page, protect_content);
        Ok(posted)
    }

    /// `edit_business_story`: `editStory` with the new file. The story returned holds its chat, id and caption.
    pub(crate) async fn edit_story(
        &self,
        client: &Client,
        connection: &str,
        story_id: i32,
        story: StoryParts<'_>,
    ) -> Result<Story> {
        let (content, caption, entities, areas) = story;
        let mut files = Vec::new();
        let text = (caption.as_str(), entities.as_slice());
        let mut body = story_body(&mut files, connection, content, text, areas, self.me()).await?;
        body.insert("story_id".into(), story_id.into());
        let edited = self.api.call_with_files::<Value>("editStory", Value::Object(body), files).await?;
        let mut edited = self.story(client, edited)?;
        edited.caption = Some(caption).filter(|caption| !caption.is_empty());
        edited.is_edited = true;
        Ok(edited)
    }

    /// `repost_business_story`: `repostStory`, which names the original by chat and story id.
    pub(crate) async fn repost_story(
        &self,
        client: &Client,
        connection: &str,
        (from_chat_id, from_story_id): (i64, i32),
        active_period: i32,
        (post_to_chat_page, protect_content): (bool, bool),
    ) -> Result<Story> {
        let mut body = Map::new();
        body.insert("business_connection_id".into(), connection.into());
        body.insert("from_chat_id".into(), from_chat_id.into());
        body.insert("from_story_id".into(), from_story_id.into());
        body.insert("active_period".into(), active_period.into());
        flag(&mut body, "post_to_chat_page", post_to_chat_page);
        flag(&mut body, "protect_content", protect_content);
        let posted = self.call("repostStory", Value::Object(body)).await?;
        let mut posted = self.story(client, posted)?;
        posted.expire_date = posted.date.saturating_add(active_period);
        posted.forward_from_story_id = Some(from_story_id);
        (posted.is_pinned, posted.has_protected_content) = (post_to_chat_page, protect_content);
        Ok(posted)
    }

    /// A story the Bot API posted, as its chat and id, posted now and visible to everyone.
    fn story(&self, client: &Client, posted: Value) -> Result<Story> {
        #[derive(Deserialize)]
        struct Posted {
            chat: bot::Chat,
            id: i64,
        }
        let posted: Posted = parse(posted)?;
        let mut peers = Peers::new(self.me());
        peers.chat(&posted.chat);
        let entities = Entities::new(client, peers.users, peers.chats);
        let mut story = Story::from_media(client, &get_peer(posted.chat.id), int(posted.id), &entities);
        story.date = int(now());
        (story.is_public, story.outgoing) = (true, true);
        Ok(story)
    }

    /// `upload_sticker_file`: `uploadStickerFile`, whose answer is a file id, as the TL document it stands for.
    pub(crate) async fn upload_sticker_file(&self, user_id: i64, file: Upload<'_>) -> Result<base::Document> {
        #[derive(Deserialize)]
        struct File {
            file_id: String,
            file_size: Option<i64>,
        }
        let mut files = Vec::new();
        let body = json!({
            "user_id": user_id,
            "sticker": attach(&mut files, file).await?,
            "sticker_format": sticker_format(file.mime_type()),
        });
        let uploaded: File = self.api.call_with_files("uploadStickerFile", body, files).await?;
        let id = FileId::decode(&uploaded.file_id)
            .map_err(|_| Error::Unexpected("uploadStickerFile returned a bad file id"))?;
        let sticker = types::DocumentAttributeSticker {
            mask: false,
            alt: String::new(),
            stickerset: base::InputStickerSet::InputStickerSetEmpty,
            mask_coords: None,
        };
        let name = types::DocumentAttributeFilename { file_name: file.name().into() };
        let attributes = vec![
            base::DocumentAttribute::DocumentAttributeSticker(Box::new(sticker)),
            base::DocumentAttribute::DocumentAttributeFilename(Box::new(name)),
        ];
        let size = uploaded.file_size.unwrap_or(0);
        let mime_type = file.mime_type().into();
        let document = types::Document::new(
            id.media_id,
            id.access_hash,
            id.file_reference,
            int(now()),
            mime_type,
            size,
            id.dc_id,
            attributes,
        );
        Ok(base::Document::Document(Box::new(document)))
    }

    /// `create_new_sticker_set`: `createNewStickerSet` with every new file in it, then the set as it now is.
    pub(crate) async fn create_sticker_set(
        &self,
        user_id: i64,
        (name, title): (&str, &str),
        stickers: Vec<InputSticker<'_>>,
        sticker_type: StickerType,
        needs_repainting: bool,
    ) -> Result<base::messages::StickerSet> {
        let mut files = Vec::new();
        let mut list = Vec::with_capacity(stickers.len());
        for sticker in stickers {
            list.push(input_sticker_json(&mut files, sticker).await?);
        }
        let sticker_type = match sticker_type {
            StickerType::Regular => "regular",
            StickerType::Mask => "mask",
            StickerType::CustomEmoji => "custom_emoji",
        };
        let mut body = Map::new();
        body.insert("user_id".into(), user_id.into());
        body.insert("name".into(), name.into());
        body.insert("title".into(), title.into());
        body.insert("stickers".into(), Value::Array(list));
        body.insert("sticker_type".into(), sticker_type.into());
        flag(&mut body, "needs_repainting", needs_repainting);
        self.api.call_with_files::<Value>("createNewStickerSet", Value::Object(body), files).await?;
        self.sticker_set(name).await
    }

    /// `add_sticker_to_set`, then the set as it now is.
    pub(crate) async fn add_sticker(
        &self,
        user_id: i64,
        name: &str,
        sticker: InputSticker<'_>,
    ) -> Result<base::messages::StickerSet> {
        let mut files = Vec::new();
        let sticker = input_sticker_json(&mut files, sticker).await?;
        let body = json!({ "user_id": user_id, "name": name, "sticker": sticker });
        self.api.call_with_files::<Value>("addStickerToSet", body, files).await?;
        self.sticker_set(name).await
    }

    /// `replace_sticker_in_set`, in the set `old` is from, then the set as it now is.
    pub(crate) async fn replace_sticker(
        &self,
        user_id: i64,
        old: &Sticker,
        sticker: InputSticker<'_>,
    ) -> Result<base::messages::StickerSet> {
        let name = set_of(old)?;
        let mut files = Vec::new();
        let sticker = input_sticker_json(&mut files, sticker).await?;
        let body = json!({ "user_id": user_id, "name": name, "old_sticker": old.file_id, "sticker": sticker });
        self.api.call_with_files::<Value>("replaceStickerInSet", body, files).await?;
        self.sticker_set(name).await
    }

    /// One of the methods that change a sticker in its set (`setStickerPositionInSet`, `deleteStickerFromSet`,
    /// `setStickerEmojiList`, `setStickerKeywords`, `setStickerMaskPosition`), with `fields` besides the sticker,
    /// then the set as it now is.
    async fn change_sticker(
        &self,
        sticker: &Sticker,
        method: &str,
        fields: Value,
    ) -> Result<base::messages::StickerSet> {
        let name = set_of(sticker)?;
        let mut body = match fields {
            Value::Object(fields) => fields,
            _ => Map::new(),
        };
        body.insert("sticker".into(), sticker.file_id.clone().into());
        self.call(method, Value::Object(body)).await?;
        self.sticker_set(name).await
    }

    /// `set_sticker_position_in_set`.
    pub(crate) async fn sticker_position(
        &self,
        sticker: &Sticker,
        position: i32,
    ) -> Result<base::messages::StickerSet> {
        self.change_sticker(sticker, "setStickerPositionInSet", json!({ "position": position })).await
    }

    /// `delete_sticker_from_set`.
    pub(crate) async fn delete_sticker(&self, sticker: &Sticker) -> Result<base::messages::StickerSet> {
        self.change_sticker(sticker, "deleteStickerFromSet", json!({})).await
    }

    /// `set_sticker_emoji_list`, with the emoji written one after another.
    pub(crate) async fn sticker_emoji(&self, sticker: &Sticker, emoji: &str) -> Result<base::messages::StickerSet> {
        self.change_sticker(sticker, "setStickerEmojiList", json!({ "emoji_list": emoji_list(emoji) })).await
    }

    /// `set_sticker_keywords`, with the keywords joined by commas; none removes them.
    pub(crate) async fn sticker_keywords(
        &self,
        sticker: &Sticker,
        keywords: &str,
    ) -> Result<base::messages::StickerSet> {
        let keywords: Vec<&str> = keywords.split(',').map(str::trim).filter(|k| !k.is_empty()).collect();
        self.change_sticker(sticker, "setStickerKeywords", json!({ "keywords": keywords })).await
    }

    /// `set_sticker_mask_position`, and `remove_sticker_mask_position` with `None`.
    pub(crate) async fn sticker_mask(
        &self,
        sticker: &Sticker,
        position: Option<&MaskPosition>,
    ) -> Result<base::messages::StickerSet> {
        let fields = match position {
            Some(position) => json!({ "mask_position": mask_position_json(position) }),
            None => json!({}),
        };
        self.change_sticker(sticker, "setStickerMaskPosition", fields).await
    }

    /// `set_sticker_set_thumbnail`: `setStickerSetThumbnail` with the file, or with none to drop the thumbnail.
    pub(crate) async fn set_sticker_set_thumbnail(
        &self,
        name: &str,
        user_id: i64,
        thumbnail: Option<Upload<'_>>,
    ) -> Result<base::messages::StickerSet> {
        let mut files = Vec::new();
        let mut body = Map::new();
        body.insert("name".into(), name.into());
        body.insert("user_id".into(), user_id.into());
        // The Bot API wants a format even with no file; a dropped thumbnail takes the static one.
        let format = thumbnail.map_or("static", |file| sticker_format(file.mime_type()));
        if let Some(file) = thumbnail {
            body.insert("thumbnail".into(), attach(&mut files, file).await?);
        }
        body.insert("format".into(), format.into());
        self.api.call_with_files::<Value>("setStickerSetThumbnail", Value::Object(body), files).await?;
        self.sticker_set(name).await
    }
}

/// Reads `upload` into `files` and returns what names it in the request.
async fn attach(files: &mut Vec<Attachment>, upload: Upload<'_>) -> Result<Value> {
    let data = match upload {
        Upload::Path(path) => tokio::fs::read(path).await?,
        Upload::Bytes { data, .. } => data.to_vec(),
    };
    let key = format!("file{}", files.len());
    let value = marker(&key);
    files.push((key, upload.name().to_owned(), data));
    Ok(value)
}

/// Sets `key` when `on`: the Bot API reads a missing flag as false.
fn flag(out: &mut Map<String, Value>, key: &str, on: bool) {
    if on {
        out.insert(key.into(), true.into());
    }
}

/// The set a sticker is from, which the Bot API's sticker methods answer about.
fn set_of(sticker: &Sticker) -> Result<&str> {
    sticker.set_name.as_deref().ok_or(Error::InvalidArgument(
        "over the Bot API, a sticker set is changed through a sticker read from it with get_sticker_set",
    ))
}

/// The Bot API's sticker format of a file's MIME type, as the friendly methods guess it from the extension.
fn sticker_format(mime_type: &str) -> &'static str {
    match mime_type {
        "application/x-tgsticker" => "animated",
        "video/webm" => "video",
        _ => "static",
    }
}

/// The fields `postStory` and `editStory` share.
async fn story_body(
    files: &mut Vec<Attachment>,
    connection: &str,
    content: InputStoryContent<'_>,
    (caption, entities): (&str, &[base::MessageEntity]),
    areas: &[StoryArea],
    me: i64,
) -> Result<Map<String, Value>> {
    let content = match content {
        InputStoryContent::Photo(photo) => json!({ "type": "photo", "photo": attach(files, photo).await? }),
        InputStoryContent::Video { video, duration, cover_frame_timestamp, is_animation } => {
            let video = attach(files, video).await?;
            json!({
                "type": "video",
                "video": video,
                "duration": duration,
                "cover_frame_timestamp": cover_frame_timestamp,
                "is_animation": is_animation,
            })
        }
    };
    let mut body = Map::new();
    body.insert("business_connection_id".into(), connection.into());
    body.insert("content".into(), content);
    if !caption.is_empty() {
        body.insert("caption".into(), caption.into());
    }
    if !entities.is_empty() {
        body.insert("caption_entities".into(), convert::entities_json(entities, me));
    }
    if !areas.is_empty() {
        body.insert("areas".into(), Value::Array(areas.iter().map(area_json).collect::<Result<_>>()?));
    }
    Ok(body)
}

/// A story area as the Bot API's `StoryArea`. Venues and channel posts have no Bot API form.
fn area_json(area: &StoryArea) -> Result<Value> {
    let p = &area.position;
    let position = json!({
        "x_percentage": p.x_percentage,
        "y_percentage": p.y_percentage,
        "width_percentage": p.width_percentage,
        "height_percentage": p.height_percentage,
        "rotation_angle": p.rotation_angle,
        "corner_radius_percentage": p.corner_radius_percentage,
    });
    let kind = match &area.kind {
        StoryAreaType::Location { latitude, longitude, address } => {
            let mut out = Map::new();
            out.insert("type".into(), "location".into());
            out.insert("latitude".into(), (*latitude).into());
            out.insert("longitude".into(), (*longitude).into());
            if let Some(address) = address {
                let mut fields = Map::new();
                fields.insert("country_code".into(), address.country_code.clone().into());
                let parts = [("state", &address.state), ("city", &address.city), ("street", &address.street)];
                for (key, part) in parts {
                    let Some(part) = part else { continue };
                    fields.insert(key.into(), part.clone().into());
                }
                out.insert("address".into(), Value::Object(fields));
            }
            Value::Object(out)
        }
        StoryAreaType::SuggestedReaction { reaction, is_dark, is_flipped } => {
            let reaction = match reaction {
                ReactionType::Emoji(emoji) => json!({ "type": "emoji", "emoji": emoji }),
                ReactionType::CustomEmoji(id) => json!({ "type": "custom_emoji", "custom_emoji_id": id.to_string() }),
                ReactionType::Paid => json!({ "type": "paid" }),
            };
            json!({ "type": "suggested_reaction", "reaction_type": reaction, "is_dark": is_dark, "is_flipped": is_flipped })
        }
        StoryAreaType::Link { url } => json!({ "type": "link", "url": url }),
        StoryAreaType::Weather { temperature, emoji, background_color } => {
            json!({ "type": "weather", "temperature": temperature, "emoji": emoji, "background_color": background_color })
        }
        StoryAreaType::UniqueGift { name } => json!({ "type": "unique_gift", "name": name }),
        StoryAreaType::Venue(_) | StoryAreaType::ChannelPost { .. } => {
            return Err(unsupported("venue and channel post story areas"));
        }
    };
    Ok(json!({ "position": position, "type": kind }))
}

/// A sticker for a set as the Bot API's `InputSticker`. A file already on Telegram goes by file id, and as a static
/// sticker: its location doesn't say what format it has.
async fn input_sticker_json(files: &mut Vec<Attachment>, sticker: InputSticker<'_>) -> Result<Value> {
    let (file, format) = match sticker.file {
        StickerFile::Upload(upload) => (attach(files, upload).await?, sticker_format(upload.mime_type())),
        StickerFile::Stored(location) => (stored_file_id(&location)?.into(), "static"),
    };
    let mut out = Map::new();
    out.insert("sticker".into(), file);
    out.insert("format".into(), format.into());
    out.insert("emoji_list".into(), emoji_list(&sticker.emoji).into());
    if let Some(position) = &sticker.mask_position {
        out.insert("mask_position".into(), mask_position_json(position));
    }
    if let Some(keywords) = &sticker.keywords {
        let keywords: Vec<&str> = keywords.split(',').map(str::trim).filter(|k| !k.is_empty()).collect();
        out.insert("keywords".into(), keywords.into());
    }
    Ok(Value::Object(out))
}

/// The Bot API's `MaskPosition`.
pub(crate) fn mask_position_json(position: &MaskPosition) -> Value {
    let point = match position.point {
        MaskPointType::Forehead => "forehead",
        MaskPointType::Eyes => "eyes",
        MaskPointType::Mouth => "mouth",
        MaskPointType::Chin => "chin",
    };
    json!({ "point": point, "x_shift": position.x_shift, "y_shift": position.y_shift, "scale": position.scale })
}

/// The Bot API file id of a sticker the client already has.
fn stored_file_id(location: &FileLocation) -> Result<String> {
    let base::InputFileLocation::InputDocumentFileLocation(l) = &location.location else {
        return Err(Error::InvalidArgument("a sticker must be a document"));
    };
    let dc_id = location.dc_id.unwrap_or_default();
    Ok(FileId::new(FileType::Sticker, dc_id, l.id, l.access_hash, &l.file_reference).encode())
}

/// The emoji of a sticker, written one after another as [`InputSticker::new`] takes them, as a list. An emoji
/// joined by U+200D, a variation selector, a skin tone, a keycap or tag characters stays one, and so does a flag's
/// pair of regional indicators.
pub(crate) fn emoji_list(emoji: &str) -> Vec<String> {
    let regional = |c: char| ('\u{1f1e6}'..='\u{1f1ff}').contains(&c);
    let mut list: Vec<String> = Vec::new();
    let mut joined = false;
    for c in emoji.chars().filter(|c| !c.is_whitespace()) {
        let extends = joined
            || matches!(c, '\u{200d}' | '\u{fe0e}' | '\u{fe0f}' | '\u{20e3}' | '\u{1f3fb}'..='\u{1f3ff}' | '\u{e0020}'..='\u{e007f}');
        let last_alone = |last: &String| last.chars().count() == 1 && last.chars().all(regional);
        let pair = regional(c) && list.last().is_some_and(last_alone);
        match list.last_mut() {
            Some(last) if extends || pair => last.push(c),
            _ => list.push(c.to_string()),
        }
        joined = c == '\u{200d}';
    }
    list
}

#[cfg(test)]
mod tests {
    use super::{emoji_list, sticker_format};

    #[test]
    fn emoji_split_one_by_one_keeping_sequences_whole() {
        // A plain emoji, one with a variation selector, a family joined by U+200D, a skin tone, and a flag.
        let list = emoji_list("😀❤️👨\u{200d}👩\u{200d}👧👍🏽🇺🇦");
        assert_eq!(list, ["😀", "❤️", "👨\u{200d}👩\u{200d}👧", "👍🏽", "🇺🇦"]);
        assert_eq!(emoji_list("🇺🇦🇵🇱"), ["🇺🇦", "🇵🇱"]);
    }

    #[test]
    fn sticker_formats_follow_the_mime_type() {
        assert_eq!(sticker_format("application/x-tgsticker"), "animated");
        assert_eq!(sticker_format("video/webm"), "video");
        assert_eq!(sticker_format("image/png"), "static");
    }
}
