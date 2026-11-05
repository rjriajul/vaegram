use crate::file_id::{FileId, FileType, ThumbnailSource, unique_id};
use crate::raw::types;
use crate::types::{AnimatedChatPhoto, ChatPhotoSticker};

/// The current photo of a user or chat. Pass a file id to `download_media`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatPhoto {
    /// Whether the photo is a short video.
    pub has_animation: bool,
    /// Whether you set this photo for the contact yourself. Only you see it.
    pub is_personal: bool,
    /// The video of an animated photo. Only full chat info, as from `get_chat`, fills it.
    pub animation: Option<Box<AnimatedChatPhoto>>,
    /// The sticker or emoji the photo was made from. Only full chat info, as from `get_chat`, fills it.
    pub sticker: Option<Box<ChatPhotoSticker>>,
    pub(crate) peer_id: i64,
    pub(crate) access_hash: i64,
    pub(crate) photo_id: i64,
    pub(crate) dc_id: i32,
}

impl ChatPhoto {
    /// `peer_id` is the Bot API id; `access_hash` is the peer's, 0 for basic groups.
    pub(crate) fn new(
        peer_id: i64,
        access_hash: i64,
        photo_id: i64,
        dc_id: i32,
        has_animation: bool,
        is_personal: bool,
    ) -> ChatPhoto {
        ChatPhoto { has_animation, is_personal, animation: None, sticker: None, peer_id, access_hash, photo_id, dc_id }
    }

    /// Adds the video and sticker the full photo carries; a photo with another id is left as it is.
    pub(crate) fn with_full(self, photo: &types::Photo) -> ChatPhoto {
        if photo.id != self.photo_id {
            return self;
        }
        ChatPhoto {
            animation: AnimatedChatPhoto::parse(photo).map(Box::new),
            sticker: ChatPhotoSticker::parse(photo).map(Box::new),
            ..self
        }
    }

    /// Returns the file id of the 160x160 version.
    pub fn small_file_id(&self) -> String {
        self.file_id(ThumbnailSource::ChatPhotoSmall { chat_id: self.peer_id, chat_access_hash: self.access_hash })
    }

    /// Returns the file id of the 640x640 version.
    pub fn big_file_id(&self) -> String {
        self.file_id(ThumbnailSource::ChatPhotoBig { chat_id: self.peer_id, chat_access_hash: self.access_hash })
    }

    /// Returns the unique id of the photo. It is the same for the small and big versions.
    pub fn small_photo_unique_id(&self) -> String {
        unique_id(self.photo_id)
    }

    /// Returns the unique id of the photo. It is the same for the small and big versions.
    pub fn big_photo_unique_id(&self) -> String {
        unique_id(self.photo_id)
    }

    fn file_id(&self, source: ThumbnailSource) -> String {
        let id = FileId {
            thumbnail_source: Some(source),
            ..FileId::new(FileType::ChatPhoto, self.dc_id, self.photo_id, 0, &[])
        };
        id.encode()
    }
}

#[cfg(test)]
mod tests {
    use super::ChatPhoto;
    use crate::enums::ChatPhotoStickerType;
    use crate::raw::{base, types};
    use crate::types::FileLocation;
    use crate::utils::get_channel_id;

    #[test]
    fn file_ids_point_at_the_peer_photo() {
        let location = |peer_id, big| {
            let photo = ChatPhoto::new(peer_id, 77, 123, 2, false, false);
            let id = if big { photo.big_file_id() } else { photo.small_file_id() };
            let location = FileLocation::try_from(&id).unwrap();
            assert_eq!(location.dc_id, Some(2));
            location.location
        };
        let expect = |big, peer| {
            let location = types::InputPeerPhotoFileLocation { big, peer, photo_id: 123 };
            base::InputFileLocation::InputPeerPhotoFileLocation(Box::new(location))
        };
        let user = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 5, access_hash: 77 }));
        assert_eq!(location(5, true), expect(true, user.clone()));
        assert_eq!(location(5, false), expect(false, user));
        let group = base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 5 });
        assert_eq!(location(-5, true), expect(true, group));
        let channel = types::InputPeerChannel { channel_id: 5, access_hash: 77 };
        let channel = base::InputPeer::InputPeerChannel(Box::new(channel));
        assert_eq!(location(get_channel_id(5), false), expect(false, channel));
        let photo = ChatPhoto::new(5, 77, 123, 2, false, false);
        assert_eq!(photo.small_photo_unique_id(), photo.big_photo_unique_id());
    }

    fn photo(id: i64, video_sizes: Option<Vec<base::VideoSize>>) -> types::Photo {
        types::Photo {
            has_stickers: false,
            id,
            access_hash: 9,
            file_reference: vec![1],
            date: 0,
            sizes: Vec::new(),
            video_sizes,
            dc_id: 2,
        }
    }

    fn video(kind: &str, side: i32, size: i32, start: Option<f64>) -> base::VideoSize {
        let v = types::VideoSize { r#type: kind.into(), w: side, h: side, size, video_start_ts: start };
        base::VideoSize::VideoSize(Box::new(v))
    }

    #[test]
    fn animation_uses_the_largest_video_size() {
        let sizes = vec![video("u", 400, 10, None), video("v", 800, 20, Some(1.5)), video("w", 100, 5, None)];
        let full = ChatPhoto::new(5, 77, 123, 2, true, false).with_full(&photo(123, Some(sizes)));
        let animation = full.animation.expect("an animation");
        assert_eq!((animation.length, animation.file_size, animation.main_frame_timestamp), (800, 20, Some(1.5)));
        assert!(full.sticker.is_none());
    }

    #[test]
    fn no_video_sizes_no_animation() {
        let plain = ChatPhoto::new(5, 77, 123, 2, false, false);
        assert_eq!(plain.clone().with_full(&photo(123, None)), plain);
    }

    #[test]
    fn with_full_keeps_file_ids() {
        let base = ChatPhoto::new(5, 77, 123, 2, true, false);
        let full = base.clone().with_full(&photo(123, Some(vec![video("u", 400, 10, None)])));
        assert_eq!((full.small_file_id(), full.big_file_id()), (base.small_file_id(), base.big_file_id()));
        let location = FileLocation::try_from(&full.animation.as_ref().unwrap().file_id).unwrap();
        let expected =
            types::InputPhotoFileLocation { id: 123, access_hash: 9, file_reference: vec![1], thumb_size: "u".into() };
        assert_eq!(location.location, base::InputFileLocation::InputPhotoFileLocation(Box::new(expected)));
        let unique = full.animation.as_ref().map(|a| a.file_unique_id.clone());
        assert_ne!(unique, Some(full.small_photo_unique_id()));
        let other_size = base.clone().with_full(&photo(123, Some(vec![video("v", 400, 10, None)])));
        assert_ne!(unique, other_size.animation.map(|a| a.file_unique_id));
        // Another photo's details never land on this one.
        let other = base.clone().with_full(&photo(124, Some(vec![video("u", 400, 10, None)])));
        assert_eq!(other, base);
    }

    #[test]
    fn sticker_from_emoji_and_set_markup() {
        let emoji = base::VideoSize::VideoSizeEmojiMarkup(Box::new(types::VideoSizeEmojiMarkup {
            emoji_id: 42,
            background_colors: vec![1],
        }));
        let set =
            base::InputStickerSet::InputStickerSetID(Box::new(types::InputStickerSetID { id: 8, access_hash: 3 }));
        let sticker = base::VideoSize::VideoSizeStickerMarkup(Box::new(types::VideoSizeStickerMarkup {
            stickerset: set,
            sticker_id: 11,
            background_colors: Vec::new(),
        }));
        let photo_of = |size| ChatPhoto::new(5, 77, 123, 2, true, false).with_full(&photo(123, Some(vec![size])));
        let from_emoji = photo_of(emoji).sticker.expect("a sticker");
        assert_eq!((from_emoji.kind, from_emoji.custom_emoji_id), (ChatPhotoStickerType::CustomEmoji, Some(42)));
        let from_set = photo_of(sticker).sticker.expect("a sticker");
        assert_eq!(
            (from_set.kind, from_set.sticker_set_id, from_set.sticker_id),
            (ChatPhotoStickerType::RegularOrMask, Some(8), Some(11))
        );
    }
}
