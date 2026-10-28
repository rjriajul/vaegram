use crate::raw::base;

/// What a message's media is. An animation is not also a document, and a voice note is not also audio.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MediaKind {
    Photo = 1,
    Video,
    Animation,
    Audio,
    Voice,
    VideoNote,
    Document,
    Sticker,
    LivePhoto,
    Contact,
    Location,
    Venue,
    Dice,
    Poll,
    Game,
    Invoice,
    Checklist,
    Story,
    Giveaway,
    PaidMedia,
    WebPage,
    Unknown,
}

impl MediaKind {
    pub(crate) fn of(media: &base::MessageMedia) -> Option<MediaKind> {
        use base::MessageMedia as M;
        Some(match media {
            M::MessageMediaEmpty => return None,
            M::MessageMediaPhoto(p) if p.live_photo => MediaKind::LivePhoto,
            M::MessageMediaPhoto(_) => MediaKind::Photo,
            M::MessageMediaDocument(d) if d.round => MediaKind::VideoNote,
            M::MessageMediaDocument(d) if d.voice => MediaKind::Voice,
            M::MessageMediaDocument(d) => MediaKind::of_document(d.document.as_ref(), d.video),
            M::MessageMediaContact(_) => MediaKind::Contact,
            M::MessageMediaGeo(_) | M::MessageMediaGeoLive(_) => MediaKind::Location,
            M::MessageMediaVenue(_) => MediaKind::Venue,
            M::MessageMediaDice(_) => MediaKind::Dice,
            M::MessageMediaPoll(_) => MediaKind::Poll,
            M::MessageMediaGame(_) => MediaKind::Game,
            M::MessageMediaInvoice(_) => MediaKind::Invoice,
            M::MessageMediaToDo(_) => MediaKind::Checklist,
            M::MessageMediaStory(_) => MediaKind::Story,
            M::MessageMediaGiveaway(_) | M::MessageMediaGiveawayResults(_) => MediaKind::Giveaway,
            M::MessageMediaPaidMedia(_) => MediaKind::PaidMedia,
            M::MessageMediaWebPage(_) => MediaKind::WebPage,
            _ => MediaKind::Unknown,
        })
    }

    /// `video` is the message's own video flag; a document outside a message has none.
    pub(crate) fn of_document(document: Option<&base::Document>, video: bool) -> MediaKind {
        use base::DocumentAttribute as A;
        let attributes = match document {
            Some(base::Document::Document(doc)) => &doc.attributes[..],
            _ => &[],
        };
        let has = |f: fn(&A) -> bool| attributes.iter().any(f);
        if has(|a| matches!(a, A::DocumentAttributeSticker(_))) {
            MediaKind::Sticker
        } else if has(|a| matches!(a, A::DocumentAttributeAnimated)) {
            MediaKind::Animation
        } else if has(|a| matches!(a, A::DocumentAttributeVideo(v) if v.round_message)) {
            MediaKind::VideoNote
        } else if video || has(|a| matches!(a, A::DocumentAttributeVideo(_))) {
            MediaKind::Video
        } else if has(|a| matches!(a, A::DocumentAttributeAudio(a) if a.voice)) {
            MediaKind::Voice
        } else if has(|a| matches!(a, A::DocumentAttributeAudio(_))) {
            MediaKind::Audio
        } else {
            MediaKind::Document
        }
    }
}
