use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::{LivePhoto, Media, Photo, StrippedThumbnail, Video};

/// Media a message sells for Stars.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct PaidMediaInfo {
    /// Price in Stars.
    pub star_count: i64,
    /// The media items. A bought item that has no media is left out.
    pub paid_media: Vec<PaidMedia>,
}

/// One item of paid media.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum PaidMedia {
    /// Until it is bought, only the size and a video's length are shown.
    #[non_exhaustive]
    Preview {
        /// Width in pixels, if Telegram sent it.
        width: Option<i32>,
        /// Height in pixels, if Telegram sent it.
        height: Option<i32>,
        /// Length of a video in seconds. `None` for photos.
        duration: Option<i32>,
        /// A blurred preview, if Telegram sent one.
        thumbnail: Option<StrippedThumbnail>,
    },
    /// A bought photo.
    Photo(Photo),
    /// A bought video.
    Video(Video),
    /// A bought live photo.
    LivePhoto(LivePhoto),
    /// A bought item of a kind this crate has no variant for.
    #[non_exhaustive]
    Unknown {
        /// TL constructor id of the item's media, for logging.
        constructor_id: u32,
    },
}

impl PaidMediaInfo {
    pub(crate) fn parse(client: &Client, raw: &types::MessageMediaPaidMedia, entities: &Entities) -> PaidMediaInfo {
        let paid_media = raw
            .extended_media
            .iter()
            .filter_map(|media| match media {
                base::MessageExtendedMedia::MessageExtendedMediaPreview(p) => {
                    let thumbnail = stripped(p.thumb.as_ref());
                    Some(PaidMedia::Preview { width: p.w, height: p.h, duration: p.video_duration, thumbnail })
                }
                base::MessageExtendedMedia::MessageExtendedMedia(m) => {
                    match Media::parse(client, &m.media, entities)? {
                        Media::Photo(photo) => Some(PaidMedia::Photo(photo)),
                        Media::Video(video) => Some(PaidMedia::Video(video)),
                        Media::LivePhoto(live) => Some(PaidMedia::LivePhoto(live)),
                        _ => Some(PaidMedia::Unknown { constructor_id: m.media.constructor_id() }),
                    }
                }
            })
            .collect();
        PaidMediaInfo { star_count: raw.stars_amount, paid_media }
    }
}

/// The data of a stripped size; a preview may also carry another kind of size, which this doesn't read.
fn stripped(size: Option<&base::PhotoSize>) -> Option<StrippedThumbnail> {
    match size? {
        base::PhotoSize::PhotoStrippedSize(s) => Some(StrippedThumbnail { data: s.bytes.clone() }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{PaidMedia, PaidMediaInfo};
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};

    #[test]
    fn reads_the_price_and_previews() {
        let preview =
            types::MessageExtendedMediaPreview { w: Some(640), h: Some(480), thumb: None, video_duration: Some(9) };
        let raw = types::MessageMediaPaidMedia {
            stars_amount: 5,
            extended_media: vec![base::MessageExtendedMedia::MessageExtendedMediaPreview(Box::new(preview))],
        };
        let info = PaidMediaInfo::parse(&Client::offline(), &raw, &Entities::default());
        assert_eq!(info.star_count, 5);
        let [PaidMedia::Preview { width, height, duration, thumbnail }] = &info.paid_media[..] else {
            panic!("{info:?}")
        };
        assert_eq!((width, height, duration), (&Some(640), &Some(480), &Some(9)));
        assert!(thumbnail.is_none());
    }

    #[test]
    fn preview_keeps_its_stripped_thumbnail() {
        let stripped = types::PhotoStrippedSize { r#type: "i".into(), bytes: vec![1, 0x0e, 0x28, 0xa3] };
        let preview = types::MessageExtendedMediaPreview {
            w: None,
            h: None,
            thumb: Some(base::PhotoSize::PhotoStrippedSize(Box::new(stripped))),
            video_duration: None,
        };
        let raw = types::MessageMediaPaidMedia {
            stars_amount: 1,
            extended_media: vec![base::MessageExtendedMedia::MessageExtendedMediaPreview(Box::new(preview))],
        };
        let info = PaidMediaInfo::parse(&Client::offline(), &raw, &Entities::default());
        let [PaidMedia::Preview { thumbnail: Some(thumbnail), .. }] = &info.paid_media[..] else { panic!("{info:?}") };
        assert_eq!(thumbnail.data, [1, 0x0e, 0x28, 0xa3]);
        assert_eq!(&thumbnail.to_jpeg()[..2], &[0xff, 0xd8]);
    }

    #[test]
    fn keeps_a_bought_live_photo() {
        let base::MessageMedia::MessageMediaDocument(video) =
            crate::types::messages_and_media::media::tests::document_media("video/mp4", Vec::new())
        else {
            panic!()
        };
        let live = types::MessageMediaPhoto {
            spoiler: false,
            live_photo: true,
            photo: None,
            ttl_seconds: None,
            video: video.document,
        };
        let media = types::MessageExtendedMedia { media: base::MessageMedia::MessageMediaPhoto(Box::new(live)) };
        let raw = types::MessageMediaPaidMedia {
            stars_amount: 5,
            extended_media: vec![base::MessageExtendedMedia::MessageExtendedMedia(Box::new(media))],
        };
        let info = PaidMediaInfo::parse(&Client::offline(), &raw, &Entities::default());
        let kinds: Vec<_> = info.paid_media.iter().map(|m| format!("{m:?}")).collect();
        assert!(matches!(&kinds[..], [live] if live.starts_with("LivePhoto")), "{kinds:?}");
    }

    #[test]
    fn a_bought_item_of_another_kind_is_unknown() {
        let document = crate::types::messages_and_media::media::tests::document_media("application/pdf", Vec::new());
        let media = types::MessageExtendedMedia { media: document };
        let raw = types::MessageMediaPaidMedia {
            stars_amount: 5,
            extended_media: vec![base::MessageExtendedMedia::MessageExtendedMedia(Box::new(media))],
        };
        let info = PaidMediaInfo::parse(&Client::offline(), &raw, &Entities::default());
        let [PaidMedia::Unknown { constructor_id }] = info.paid_media[..] else { panic!("{info:?}") };
        assert_eq!(constructor_id, 0x52d8_ccd9);
    }
}
