use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::{LivePhoto, Media, Photo, Video};

#[derive(Debug, Clone)]
pub struct PaidMediaInfo {
    pub star_count: i64,
    pub paid_media: Vec<PaidMedia>,
}

#[derive(Debug, Clone)]
pub enum PaidMedia {
    /// Until it is bought, only the size and a video's length are shown.
    Preview {
        width: Option<i32>,
        height: Option<i32>,
        duration: Option<i32>,
    },
    Photo(Photo),
    Video(Video),
    LivePhoto(LivePhoto),
}

impl PaidMediaInfo {
    pub(crate) fn parse(raw: &types::MessageMediaPaidMedia, entities: &Entities) -> PaidMediaInfo {
        let paid_media = raw
            .extended_media
            .iter()
            .filter_map(|media| match media {
                base::MessageExtendedMedia::MessageExtendedMediaPreview(p) => {
                    Some(PaidMedia::Preview { width: p.w, height: p.h, duration: p.video_duration })
                }
                base::MessageExtendedMedia::MessageExtendedMedia(m) => match Media::parse(&m.media, entities)? {
                    Media::Photo(photo) => Some(PaidMedia::Photo(photo)),
                    Media::Video(video) => Some(PaidMedia::Video(video)),
                    Media::LivePhoto(live) => Some(PaidMedia::LivePhoto(live)),
                    _ => None,
                },
            })
            .collect();
        PaidMediaInfo { star_count: raw.stars_amount, paid_media }
    }
}

#[cfg(test)]
mod tests {
    use super::{PaidMedia, PaidMediaInfo};
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
        let info = PaidMediaInfo::parse(&raw, &Entities::default());
        assert_eq!(info.star_count, 5);
        let [PaidMedia::Preview { width, height, duration }] = info.paid_media[..] else { panic!("{info:?}") };
        assert_eq!((width, height, duration), (Some(640), Some(480), Some(9)));
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
        let info = PaidMediaInfo::parse(&raw, &Entities::default());
        let kinds: Vec<_> = info.paid_media.iter().map(|m| format!("{m:?}")).collect();
        assert!(matches!(&kinds[..], [live] if live.starts_with("LivePhoto")), "{kinds:?}");
    }
}
