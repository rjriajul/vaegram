//! Stickers: sets read by name, custom emoji, and the set changes TL names by set name. Changes that name a sticker
//! go through the friendly methods' own branches ([`super::uploads`]), as they need its Bot API file id.

use serde::Deserialize;
use serde_json::{Value, json};

use super::{done, parse};
use crate::bot_api::bridge::{Transport, unsupported};
use crate::bot_api::convert::int;
use crate::bot_api::raw::types as bot;
use crate::errors::Result;
use crate::file_id::{FileId, ThumbnailSource};
use crate::raw::{Deserializable, Identifiable, Serializable, base, functions as f, types};
use crate::utils::input_sticker_set;

const GET_STICKER_SET: u32 = <f::messages::GetStickerSet as Identifiable>::CONSTRUCTOR_ID;
const CUSTOM_EMOJI: u32 = <f::messages::GetCustomEmojiDocuments as Identifiable>::CONSTRUCTOR_ID;
const RENAME_SET: u32 = <f::stickers::RenameStickerSet as Identifiable>::CONSTRUCTOR_ID;
const DELETE_SET: u32 = <f::stickers::DeleteStickerSet as Identifiable>::CONSTRUCTOR_ID;
const SET_THUMB: u32 = <f::stickers::SetStickerSetThumb as Identifiable>::CONSTRUCTOR_ID;

/// A Bot API `Sticker`, with the fields a TL document has a place for.
#[derive(Deserialize)]
pub(super) struct Sticker {
    file_id: String,
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    width: i64,
    #[serde(default)]
    height: i64,
    #[serde(default)]
    is_animated: bool,
    #[serde(default)]
    is_video: bool,
    thumbnail: Option<bot::PhotoSize>,
    emoji: Option<String>,
    set_name: Option<String>,
    mask_position: Option<MaskPosition>,
    #[serde(default)]
    needs_repainting: bool,
    file_size: Option<i64>,
}

#[derive(Deserialize)]
struct MaskPosition {
    point: String,
    #[serde(default)]
    x_shift: f64,
    #[serde(default)]
    y_shift: f64,
    #[serde(default)]
    scale: f64,
}

/// A Bot API `StickerSet`.
#[derive(Deserialize)]
struct Set {
    name: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    sticker_type: String,
    #[serde(default)]
    stickers: Vec<Sticker>,
}

impl Transport {
    pub(super) async fn answer_stickers(&self, id: u32, body: &[u8]) -> Option<Result<Vec<u8>>> {
        Some(match id {
            GET_STICKER_SET => self.get_sticker_set(body).await,
            CUSTOM_EMOJI => self.custom_emoji_stickers(body).await,
            RENAME_SET => self.rename_sticker_set(body).await,
            DELETE_SET => self.delete_sticker_set(body).await,
            SET_THUMB => self.set_custom_emoji_thumbnail(body).await,
            _ => return None,
        })
    }

    async fn get_sticker_set(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::messages::GetStickerSet::from_bytes(body)?;
        Ok(self.sticker_set(set_name(&r.stickerset)?).await?.to_bytes())
    }

    /// The set named `name` as TL's `messages.stickerSet`. The Bot API gives a set no id or access hash, so both
    /// are 0, and each sticker's emoji is the one pack it is in.
    pub(crate) async fn sticker_set(&self, name: &str) -> Result<base::messages::StickerSet> {
        let set: Set = parse(self.call("getStickerSet", json!({ "name": name })).await?)?;
        let mut documents = Vec::with_capacity(set.stickers.len());
        let mut packs: Vec<(String, Vec<i64>)> = Vec::new();
        for sticker in &set.stickers {
            let Some(document) = sticker_document(sticker) else { continue };
            if let (Some(emoji), base::Document::Document(d)) = (&sticker.emoji, &document) {
                match packs.iter_mut().find(|(known, _)| known == emoji) {
                    Some((_, ids)) => ids.push(d.id),
                    None => packs.push((emoji.clone(), vec![d.id])),
                }
            }
            documents.push(document);
        }
        let count = i32::try_from(documents.len()).unwrap_or(i32::MAX);
        let mut raw = types::StickerSet::new(0, 0, set.title, set.name, count, 0);
        raw.masks = set.sticker_type == "mask";
        raw.emojis = set.sticker_type == "custom_emoji";
        let packs = packs
            .into_iter()
            .map(|(emoticon, documents)| {
                base::StickerPack::StickerPack(Box::new(types::StickerPack { emoticon, documents }))
            })
            .collect();
        let set = types::messages::StickerSet {
            set: base::StickerSet::StickerSet(Box::new(raw)),
            packs,
            keywords: Vec::new(),
            documents,
        };
        Ok(base::messages::StickerSet::StickerSet(Box::new(set)))
    }

    async fn custom_emoji_stickers(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::messages::GetCustomEmojiDocuments::from_bytes(body)?;
        let ids: Vec<String> = r.document_id.iter().map(i64::to_string).collect();
        let stickers: Vec<Sticker> =
            parse(self.call("getCustomEmojiStickers", json!({ "custom_emoji_ids": ids })).await?)?;
        let documents: Vec<base::Document> = stickers.iter().filter_map(sticker_document).collect();
        Ok(documents.to_bytes())
    }

    async fn rename_sticker_set(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::stickers::RenameStickerSet::from_bytes(body)?;
        let name = set_name(&r.stickerset)?;
        self.call("setStickerSetTitle", json!({ "name": name, "title": r.title })).await?;
        Ok(self.sticker_set(name).await?.to_bytes())
    }

    async fn delete_sticker_set(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::stickers::DeleteStickerSet::from_bytes(body)?;
        self.call("deleteStickerSet", json!({ "name": set_name(&r.stickerset)? })).await?;
        Ok(done())
    }

    /// Only a custom emoji set's thumbnail, which TL names by document id; a regular set's goes through
    /// `set_sticker_set_thumbnail`'s own branch.
    async fn set_custom_emoji_thumbnail(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::stickers::SetStickerSetThumb::from_bytes(body)?;
        let (None, Some(document_id)) = (&r.thumb, r.thumb_document_id) else {
            return Err(unsupported("setting a set's thumbnail from a TL document"));
        };
        let name = set_name(&r.stickerset)?;
        let mut out = serde_json::Map::new();
        out.insert("name".into(), name.into());
        // No custom emoji falls back to the set's first one.
        if document_id != 0 {
            out.insert("custom_emoji_id".into(), document_id.to_string().into());
        }
        self.call("setCustomEmojiStickerSetThumbnail", Value::Object(out)).await?;
        Ok(self.sticker_set(name).await?.to_bytes())
    }
}

/// The short name an input sticker set names; the Bot API knows sets by nothing else.
fn set_name(set: &base::InputStickerSet) -> Result<&str> {
    match set {
        base::InputStickerSet::InputStickerSetShortName(s) => Ok(&s.short_name),
        _ => Err(unsupported("sticker sets named other than by their short name")),
    }
}

/// A Bot API sticker as the TL document its file id stands for; `None` when the id doesn't decode.
pub(super) fn sticker_document(sticker: &Sticker) -> Option<base::Document> {
    let id = FileId::decode(&sticker.file_id).ok()?;
    let stickerset = sticker.set_name.as_deref().map_or(base::InputStickerSet::InputStickerSetEmpty, input_sticker_set);
    let alt = sticker.emoji.clone().unwrap_or_default();
    let kind = if sticker.kind == "custom_emoji" {
        let emoji =
            types::DocumentAttributeCustomEmoji { free: true, text_color: sticker.needs_repainting, alt, stickerset };
        base::DocumentAttribute::DocumentAttributeCustomEmoji(Box::new(emoji))
    } else {
        let mask_coords = sticker.mask_position.as_ref().map(|m| {
            let n = match m.point.as_str() {
                "forehead" => 0,
                "eyes" => 1,
                "mouth" => 2,
                _ => 3,
            };
            base::MaskCoords::MaskCoords(Box::new(types::MaskCoords { n, x: m.x_shift, y: m.y_shift, zoom: m.scale }))
        });
        let mask = sticker.kind == "mask";
        let attribute = types::DocumentAttributeSticker { mask, alt, stickerset, mask_coords };
        base::DocumentAttribute::DocumentAttributeSticker(Box::new(attribute))
    };
    let size = types::DocumentAttributeImageSize { w: int(sticker.width), h: int(sticker.height) };
    let mime_type = match (sticker.is_animated, sticker.is_video) {
        (true, _) => "application/x-tgsticker",
        (_, true) => "video/webm",
        _ => "image/webp",
    };
    let attributes = vec![kind, base::DocumentAttribute::DocumentAttributeImageSize(size)];
    let size = sticker.file_size.unwrap_or(0);
    let mut document = types::Document::new(
        id.media_id,
        id.access_hash,
        id.file_reference,
        0,
        mime_type.into(),
        size,
        id.dc_id,
        attributes,
    );
    document.thumbs = sticker.thumbnail.as_ref().and_then(thumbnail).map(|size| vec![size]);
    Some(base::Document::Document(Box::new(document)))
}

/// A sticker's thumbnail, named by the letter its file id carries.
fn thumbnail(photo: &bot::PhotoSize) -> Option<base::PhotoSize> {
    let letter = match FileId::decode(&photo.file_id).ok()?.thumbnail_source {
        Some(ThumbnailSource::Thumbnail { size, .. }) => size,
        _ => 'm',
    };
    Some(base::PhotoSize::PhotoSize(Box::new(types::PhotoSize {
        r#type: letter.to_string(),
        w: int(photo.width),
        h: int(photo.height),
        size: photo.file_size.map_or(0, int),
    })))
}
