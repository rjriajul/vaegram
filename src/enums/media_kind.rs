use crate::raw::base;
use crate::types::{LivePhoto, Photo};

/// What a message's media is. An animation is not also a document, and a voice note is not also audio.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MediaKind {
    /// A photo.
    Photo = 1,
    /// A video.
    Video,
    /// An animation, such as a GIF.
    Animation,
    /// An audio file, such as music.
    Audio,
    /// A voice note.
    Voice,
    /// A round video message.
    VideoNote,
    /// A file that is none of the other kinds.
    Document,
    /// A sticker.
    Sticker,
    /// A live photo.
    LivePhoto,
    /// A shared contact.
    Contact,
    /// A location, live or not.
    Location,
    /// A venue.
    Venue,
    /// A dice or other animated emoji that lands on a random result.
    Dice,
    /// A poll.
    Poll,
    /// A game.
    Game,
    /// An invoice.
    Invoice,
    /// A checklist.
    Checklist,
    /// A forwarded story.
    Story,
    /// A giveaway announcement.
    Giveaway,
    /// The winners of a giveaway.
    GiveawayWinners,
    /// Paid media.
    PaidMedia,
    /// A web page preview.
    WebPage,
    /// Media of a kind this library doesn't know.
    Unknown,
    /// An interactive widget.
    Widget,
}

// A media filter keeps its kinds in a `u32` mask.
const _: () = assert!((MediaKind::Widget as u8) < 32);

impl MediaKind {
    pub(crate) fn of(media: &base::MessageMedia) -> Option<MediaKind> {
        use base::MessageMedia as M;
        Some(match media {
            M::MessageMediaEmpty => return None,
            M::MessageMediaPhoto(p) if LivePhoto::is_live(p) => MediaKind::LivePhoto,
            // Expired media has no file left: it is no media at all, as `Media::parse` sees it.
            M::MessageMediaPhoto(p) if p.photo.as_ref().is_some_and(Photo::is_downloadable) => MediaKind::Photo,
            M::MessageMediaPhoto(_) => return None,
            M::MessageMediaDocument(d) if !matches!(d.document, Some(base::Document::Document(_))) => return None,
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
            M::MessageMediaGiveaway(_) => MediaKind::Giveaway,
            M::MessageMediaGiveawayResults(_) => MediaKind::GiveawayWinners,
            M::MessageMediaPaidMedia(_) => MediaKind::PaidMedia,
            M::MessageMediaWidget(_) => MediaKind::Widget,
            M::MessageMediaWebPage(_) => MediaKind::WebPage,
            _ => MediaKind::Unknown,
        })
    }

    /// `video` is the message's own video flag. A document outside a message has none.
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
