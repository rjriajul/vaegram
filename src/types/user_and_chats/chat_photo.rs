use crate::file_id::{FileId, FileType, ThumbnailSource, unique_id};

/// The current photo of a user or chat. Pass a file id to `download_media`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatPhoto {
    /// The photo is a short video.
    pub has_animation: bool,
    /// Set by you for this contact only; others see another photo.
    pub is_personal: bool,
    pub(crate) peer_id: i64,
    pub(crate) access_hash: i64,
    pub(crate) photo_id: i64,
    pub(crate) dc_id: i32,
}

impl ChatPhoto {
    /// `peer_id` is the Bot API id; `access_hash` is the peer's, 0 for basic groups.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn new(
        peer_id: i64,
        access_hash: i64,
        photo_id: i64,
        dc_id: i32,
        has_animation: bool,
        is_personal: bool,
    ) -> ChatPhoto {
        ChatPhoto { has_animation, is_personal, peer_id, access_hash, photo_id, dc_id }
    }

    /// The 160x160 version.
    pub fn small_file_id(&self) -> String {
        self.file_id(ThumbnailSource::ChatPhotoSmall { chat_id: self.peer_id, chat_access_hash: self.access_hash })
    }

    /// The 640x640 version.
    pub fn big_file_id(&self) -> String {
        self.file_id(ThumbnailSource::ChatPhotoBig { chat_id: self.peer_id, chat_access_hash: self.access_hash })
    }

    pub fn small_photo_unique_id(&self) -> String {
        unique_id(self.photo_id)
    }

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
}
