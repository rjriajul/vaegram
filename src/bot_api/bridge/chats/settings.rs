//! Chat settings and lookups: title, photo, description, group sticker set, verification, a user's boosts,
//! profile photos and personal channel messages, and the share theme backdrops.

use serde_json::{Map, Value, json};

use super::super::{Transport, channel_chat_id, unsupported};
use super::{decode, no_updates, put, user_of};
use crate::bot_api::convert::{self, Peers, int};
use crate::bot_api::input_file::marker;
use crate::bot_api::raw::types as bot;
use crate::errors::Result;
use crate::file_id::{FileId, ThumbnailSource};
use crate::raw::{Deserializable, Identifiable, Serializable, base, functions as f, types};
use crate::types::Upload;

const EDIT_CHAT_TITLE: u32 = <f::messages::EditChatTitle as Identifiable>::CONSTRUCTOR_ID;
const EDIT_TITLE: u32 = <f::channels::EditTitle as Identifiable>::CONSTRUCTOR_ID;
const EDIT_CHAT_PHOTO: u32 = <f::messages::EditChatPhoto as Identifiable>::CONSTRUCTOR_ID;
const EDIT_PHOTO: u32 = <f::channels::EditPhoto as Identifiable>::CONSTRUCTOR_ID;
const EDIT_ABOUT: u32 = <f::messages::EditChatAbout as Identifiable>::CONSTRUCTOR_ID;
const SET_STICKERS: u32 = <f::channels::SetStickers as Identifiable>::CONSTRUCTOR_ID;
const SET_VERIFICATION: u32 = <f::bots::SetCustomVerification as Identifiable>::CONSTRUCTOR_ID;
const GET_USER_BOOSTS: u32 = <f::premium::GetUserBoosts as Identifiable>::CONSTRUCTOR_ID;
const GET_USER_PHOTOS: u32 = <f::photos::GetUserPhotos as Identifiable>::CONSTRUCTOR_ID;
const GET_PERSONAL_HISTORY: u32 = <f::messages::GetPersonalChannelHistory as Identifiable>::CONSTRUCTOR_ID;
const GET_BACKDROPS: u32 = <f::messages::GetShareThemeBackdrops as Identifiable>::CONSTRUCTOR_ID;

impl Transport {
    pub(super) async fn answer_chat_settings(&self, id: u32, body: &[u8]) -> Option<Result<Vec<u8>>> {
        Some(match id {
            EDIT_CHAT_TITLE => self.chat_title(body, false).await,
            EDIT_TITLE => self.chat_title(body, true).await,
            EDIT_CHAT_PHOTO => self.chat_photo(body, false).await,
            EDIT_PHOTO => self.chat_photo(body, true).await,
            EDIT_ABOUT => self.chat_about(body).await,
            SET_STICKERS => self.chat_stickers(body).await,
            SET_VERIFICATION => self.custom_verification(body).await,
            GET_USER_BOOSTS => self.user_boosts(body).await,
            GET_USER_PHOTOS => self.user_photos(body).await,
            GET_PERSONAL_HISTORY => self.personal_channel(body).await,
            GET_BACKDROPS => self.share_backdrops().await,
            _ => return None,
        })
    }

    /// `messages.editChatTitle` for a basic group, `channels.editTitle` for a channel.
    async fn chat_title(&self, body: &[u8], channel: bool) -> Result<Vec<u8>> {
        let (chat_id, title) = if channel {
            let r = f::channels::EditTitle::from_bytes(body)?;
            (channel_chat_id(&r.channel)?, r.title)
        } else {
            let r = f::messages::EditChatTitle::from_bytes(body)?;
            (r.chat_id.wrapping_neg(), r.title)
        };
        self.call("setChatTitle", json!({ "chat_id": chat_id, "title": title })).await?;
        Ok(no_updates(self.me()))
    }

    /// Only a removal goes through TL: a new photo is uploaded along with `setChatPhoto` by `set_chat_photo`'s own
    /// branch, as the Bot API has no `upload.saveFilePart`.
    async fn chat_photo(&self, body: &[u8], channel: bool) -> Result<Vec<u8>> {
        let (chat_id, photo) = if channel {
            let r = f::channels::EditPhoto::from_bytes(body)?;
            (channel_chat_id(&r.channel)?, r.photo)
        } else {
            let r = f::messages::EditChatPhoto::from_bytes(body)?;
            (r.chat_id.wrapping_neg(), r.photo)
        };
        if !matches!(photo, base::InputChatPhoto::InputChatPhotoEmpty) {
            return Err(unsupported("setting a chat photo uploaded over MTProto or already on Telegram"));
        }
        self.call("deleteChatPhoto", json!({ "chat_id": chat_id })).await?;
        Ok(no_updates(self.me()))
    }

    /// `set_chat_photo`: the photo goes up in the `setChatPhoto` request itself. The Bot API sets still photos only.
    pub(crate) async fn set_chat_photo(&self, chat_id: i64, photo: Upload<'_>, video: bool) -> Result<()> {
        if video {
            return Err(unsupported("video chat photos"));
        }
        let data = match photo {
            Upload::Path(path) => tokio::fs::read(path).await?,
            Upload::Bytes { data, .. } => data.to_vec(),
        };
        let body = json!({ "chat_id": chat_id, "photo": marker("photo") });
        let upload = ("photo".to_owned(), photo.name().to_owned(), data);
        let _: Value = self.api.call_with_files("setChatPhoto", body, vec![upload]).await?;
        Ok(())
    }

    async fn chat_about(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::messages::EditChatAbout::from_bytes(body)?;
        self.call("setChatDescription", json!({ "chat_id": self.chat_id(&r.peer)?, "description": r.about })).await?;
        Ok(true.to_bytes())
    }

    /// The Bot API names a group sticker set by its short name only.
    async fn chat_stickers(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::channels::SetStickers::from_bytes(body)?;
        let chat_id = channel_chat_id(&r.channel)?;
        match r.stickerset {
            base::InputStickerSet::InputStickerSetEmpty => {
                self.call("deleteChatStickerSet", json!({ "chat_id": chat_id })).await?;
            }
            base::InputStickerSet::InputStickerSetShortName(set) => {
                let body = json!({ "chat_id": chat_id, "sticker_set_name": set.short_name });
                self.call("setChatStickerSet", body).await?;
            }
            _ => return Err(unsupported("naming a group sticker set other than by its short name")),
        }
        Ok(true.to_bytes())
    }

    /// A positive id is a user, verified with `verifyUser`; any other chat with `verifyChat`. Verifying on behalf
    /// of another bot has no Bot API counterpart.
    async fn custom_verification(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::bots::SetCustomVerification::from_bytes(body)?;
        if r.bot.is_some() {
            return Err(unsupported("verifying on behalf of another bot"));
        }
        let id = self.chat_id(&r.peer)?;
        let (key, verify, remove) = if id > 0 {
            ("user_id", "verifyUser", "removeUserVerification")
        } else {
            ("chat_id", "verifyChat", "removeChatVerification")
        };
        let mut body = Map::new();
        body.insert(key.into(), id.into());
        if !r.enabled {
            self.call(remove, Value::Object(body)).await?;
            return Ok(true.to_bytes());
        }
        if let Some(base::TextWithEntities::TextWithEntities(text)) = r.custom_description {
            if !text.entities.is_empty() {
                body.insert("custom_description_entities".into(), convert::entities_json(&text.entities, self.me()));
            }
            body.insert("custom_description".into(), text.text.into());
        }
        self.call(verify, Value::Object(body)).await?;
        Ok(true.to_bytes())
    }

    async fn user_boosts(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::premium::GetUserBoosts::from_bytes(body)?;
        let me = self.me();
        let body = json!({ "chat_id": self.chat_id(&r.peer)?, "user_id": user_of(&r.user_id, me)? });
        let list: bot::UserChatBoosts = decode(self.call("getUserChatBoosts", body).await?)?;
        let mut peers = Peers::new(me);
        let boosts: Vec<_> = list.boosts.into_iter().filter_map(|b| boost(b, &mut peers)).collect();
        let count = i32::try_from(boosts.len()).unwrap_or(i32::MAX);
        Ok(types::premium::BoostsList::new(count, boosts, peers.users).to_bytes())
    }

    /// `photos.getUserPhotos` reads by offset and limit as `getUserProfilePhotos` does; `max_id` has no counterpart
    /// and is ignored. The Bot API gives no upload date, so each photo's is 0.
    async fn user_photos(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::photos::GetUserPhotos::from_bytes(body)?;
        let mut body = Map::new();
        body.insert("user_id".into(), user_of(&r.user_id, self.me())?.into());
        body.insert("offset".into(), r.offset.into());
        put(&mut body, "limit", (r.limit > 0).then_some(r.limit));
        let found: bot::UserProfilePhotos = decode(self.call("getUserProfilePhotos", Value::Object(body)).await?)?;
        let photos = found.photos.iter().map(Vec::as_slice).filter_map(profile_photo).collect();
        let slice = types::photos::PhotosSlice { count: int(found.total_count), photos, users: Vec::new() };
        Ok(slice.to_bytes())
    }

    /// `max_id` and `min_id` have no Bot API counterpart and are ignored.
    async fn personal_channel(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::messages::GetPersonalChannelHistory::from_bytes(body)?;
        let me = self.me();
        let body = json!({ "user_id": user_of(&r.user_id, me)?, "limit": r.limit });
        let found: Vec<bot::Message> = decode(self.call("getUserPersonalChatMessages", body).await?)?;
        let mut peers = Peers::new(me);
        let messages =
            found.into_iter().map(|m| base::Message::Message(Box::new(convert::message(m, &mut peers)))).collect();
        let (chats, users) = (peers.chats, peers.users);
        Ok(types::messages::Messages { messages, topics: Vec::new(), chats, users }.to_bytes())
    }

    async fn share_backdrops(&self) -> Result<Vec<u8>> {
        let found: bot::ChatShareThemeBackdrops = decode(self.call("getChatShareThemeBackdrops", json!({})).await?)?;
        let backdrops = found
            .backdrops
            .into_iter()
            .map(|b| {
                base::ShareThemeBackdropOption::ShareThemeBackdropOption(Box::new(types::ShareThemeBackdropOption {
                    backdrop_id: int(b.id),
                    name: b.name,
                    center_color: int(b.colors.center_color),
                    edge_color: int(b.colors.edge_color),
                    pattern_color: int(b.colors.symbol_color),
                    text_color: int(b.colors.text_color),
                }))
            })
            .collect();
        Ok(types::ShareThemeBackdrops { hash: 0, backdrops }.to_bytes())
    }
}

/// A Bot API `ChatBoost` as TL's, its booster added to `peers`; `None` for a source newer than this mapping.
fn boost(b: bot::ChatBoost, peers: &mut Peers) -> Option<base::Boost> {
    let mut raw = types::Boost::new(b.boost_id, int(b.add_date), int(b.expiration_date));
    match b.source {
        bot::ChatBoostSource::Premium(source) => {
            peers.user(&source.user);
            raw.user_id = Some(source.user.id);
        }
        bot::ChatBoostSource::GiftCode(source) => {
            peers.user(&source.user);
            raw.user_id = Some(source.user.id);
            raw.gift = true;
        }
        bot::ChatBoostSource::Giveaway(source) => {
            // `giveaway` and `giveaway_msg_id` share a flag: one set without the other would be read back short.
            raw.giveaway = true;
            raw.giveaway_msg_id = Some(int(source.giveaway_message_id));
            if let Some(user) = &source.user {
                peers.user(user);
                raw.user_id = Some(user.id);
            }
            raw.stars = source.prize_star_count;
            raw.unclaimed = source.is_unclaimed.unwrap_or(false);
        }
        bot::ChatBoostSource::Unknown => return None,
    }
    Some(base::Boost::Boost(Box::new(raw)))
}

/// A profile photo from its sizes, smallest first as the Bot API lists them; `None` when its file id can't be read.
fn profile_photo(sizes: &[bot::PhotoSize]) -> Option<base::Photo> {
    let id = FileId::decode(&sizes.last()?.file_id).ok()?;
    let sizes = sizes.iter().filter_map(photo_size).collect();
    let photo = types::Photo::new(id.media_id, id.access_hash, id.file_reference, 0, sizes, id.dc_id);
    Some(base::Photo::Photo(Box::new(photo)))
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

#[cfg(test)]
mod tests {
    use super::photo_size;
    use crate::bot_api::raw::types as bot;
    use crate::raw::base;

    /// A Bot API photo id names its size with a letter, which TL's sizes carry and a friendly photo's file id reads.
    #[test]
    fn a_profile_photo_size_keeps_the_letter_of_its_file_id() {
        let size = bot::PhotoSize {
            file_id: "AgACAgIAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAIG9DEbXX3wSq3oI7t_PqQGAQADAgADbQADNgQ".into(),
            file_unique_id: "p".into(),
            width: 320,
            height: 240,
            file_size: Some(9),
        };
        let Some(base::PhotoSize::PhotoSize(raw)) = photo_size(&size) else { panic!("no size") };
        assert_eq!((raw.r#type.as_str(), raw.w, raw.h, raw.size), ("m", 320, 240, 9));
    }
}
