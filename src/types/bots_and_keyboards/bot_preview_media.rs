use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::base;
use crate::types::messages_and_media::Source;
use crate::types::{FileSource, Media};

#[derive(Debug, Clone)]
pub struct BotPreviewMedia {
    pub date: i32,
    pub media: Option<Media>,
}

impl BotPreviewMedia {
    pub(crate) fn parse(raw: base::BotPreviewMedia) -> BotPreviewMedia {
        let base::BotPreviewMedia::BotPreviewMedia(r) = raw;
        BotPreviewMedia { date: r.date, media: Media::parse(&r.media, &Entities::default()) }
    }

    /// The stored reference Telegram wants to identify this preview in edits, deletes and reorders.
    pub(crate) fn input(&self) -> Result<base::InputMedia> {
        let source = match &self.media {
            Some(Media::Photo(photo)) => FileSource::from(photo),
            Some(Media::Video(video)) => FileSource::from(video),
            Some(Media::Animation(animation)) => FileSource::from(animation),
            Some(Media::Document(document)) => FileSource::from(document),
            _ => return Err(Error::InvalidArgument("this preview holds no photo or video")),
        };
        match source.0 {
            Source::Stored(media) => Ok(media),
            _ => Err(Error::Unexpected("a stored file turned into a new upload")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BotPreviewMedia;
    use crate::errors::Error;
    use crate::raw::{base, types};

    fn preview(media: base::MessageMedia) -> BotPreviewMedia {
        let raw = types::BotPreviewMedia { date: 5, media };
        BotPreviewMedia::parse(base::BotPreviewMedia::BotPreviewMedia(Box::new(raw)))
    }

    #[test]
    fn photos_come_back_as_stored_references() {
        let size = types::PhotoSize { r#type: "x".into(), w: 10, h: 20, size: 30 };
        let photo = types::Photo {
            has_stickers: false,
            id: 1,
            access_hash: 2,
            file_reference: vec![3],
            date: 4,
            sizes: vec![base::PhotoSize::PhotoSize(Box::new(size))],
            video_sizes: None,
            dc_id: 2,
        };
        let media = types::MessageMediaPhoto {
            spoiler: false,
            live_photo: false,
            photo: Some(base::Photo::Photo(Box::new(photo))),
            ttl_seconds: None,
            video: None,
        };
        let preview = preview(base::MessageMedia::MessageMediaPhoto(Box::new(media)));
        assert_eq!(preview.date, 5);
        let Ok(base::InputMedia::InputMediaPhoto(stored)) = preview.input() else { panic!("not a stored photo") };
        let id = base::InputPhoto::InputPhoto(Box::new(types::InputPhoto {
            id: 1,
            access_hash: 2,
            file_reference: vec![3],
        }));
        assert_eq!(stored.id, id);
    }

    #[test]
    fn a_preview_without_a_photo_or_video_has_no_reference() {
        let empty = preview(base::MessageMedia::MessageMediaEmpty);
        assert!(empty.media.is_none());
        assert!(matches!(empty.input(), Err(Error::InvalidArgument(_))));
    }
}
