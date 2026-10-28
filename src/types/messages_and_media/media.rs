use crate::dispatcher::Entities;
use crate::enums::MediaKind;
use crate::raw::base;
use crate::types::{
    Animation, Audio, Checklist, Contact, Dice, Document, FileLocation, Game, Invoice, Location, Message, Photo, Poll,
    Sticker, Venue, Video, VideoNote, Voice,
};

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Media {
    Photo(Photo),
    Document(Document),
    Animation(Animation),
    Audio(Audio),
    Video(Video),
    VideoNote(VideoNote),
    Voice(Voice),
    Sticker(Sticker),
    Contact(Contact),
    Location(Location),
    Venue(Venue),
    Dice(Dice),
    Poll(Poll),
    Invoice(Invoice),
    Checklist(Checklist),
    Game(Game),
}

impl Media {
    pub(crate) fn parse(media: &base::MessageMedia, entities: &Entities) -> Option<Media> {
        use base::MessageMedia as M;
        let kind = MediaKind::of(media)?;
        Some(match media {
            M::MessageMediaPhoto(m) => Media::Photo(Photo::parse(m.photo.as_ref()?)?),
            M::MessageMediaDocument(m) => Media::parse_document(kind, m.document.as_ref()?)?,
            M::MessageMediaGeo(m) => Media::Location(Location::parse(&m.geo)?),
            M::MessageMediaGeoLive(m) => Media::Location(Location {
                live_period: Some(m.period),
                heading: m.heading,
                proximity_alert_radius: m.proximity_notification_radius,
                ..Location::parse(&m.geo)?
            }),
            M::MessageMediaContact(m) => Media::Contact(Contact::parse(m)),
            M::MessageMediaVenue(m) => Media::Venue(Venue::parse(m)?),
            M::MessageMediaDice(m) => Media::Dice(Dice { emoji: m.emoticon.clone(), value: m.value }),
            M::MessageMediaPoll(m) => Media::Poll(Poll::parse(&m.poll, &m.results, entities)),
            M::MessageMediaInvoice(m) => Media::Invoice(Invoice::parse(m)),
            M::MessageMediaToDo(m) => Media::Checklist(Checklist::parse(m, entities)),
            M::MessageMediaGame(m) => Media::Game(Game::parse(&m.game)),
            _ => return None,
        })
    }

    pub(crate) fn from_document(document: &base::Document) -> Option<Media> {
        Media::parse_document(MediaKind::of_document(Some(document), false), document)
    }

    /// Builds the type `MediaKind::of` picked, so a filter and the parsed message never disagree.
    fn parse_document(kind: MediaKind, document: &base::Document) -> Option<Media> {
        let base::Document::Document(raw) = document else { return None };
        let (mut video, mut audio, mut size) = (None, None, None);
        for attribute in &raw.attributes {
            match attribute {
                base::DocumentAttribute::DocumentAttributeVideo(a) => video = Some(&**a),
                base::DocumentAttribute::DocumentAttributeAudio(a) => audio = Some(&**a),
                base::DocumentAttribute::DocumentAttributeImageSize(a) => size = Some((a.w, a.h)),
                _ => {}
            }
        }
        let document = Document::parse(document)?;
        Some(match kind {
            MediaKind::Sticker => Media::Sticker(Sticker::from_document(document, &raw.attributes)?),
            MediaKind::Animation => Media::Animation(Animation::parse(document, video, size)),
            MediaKind::VideoNote => Media::VideoNote(VideoNote::parse(document, video)),
            MediaKind::Video => Media::Video(Video::parse(document, video)),
            MediaKind::Voice => Media::Voice(Voice::parse(document, audio)),
            MediaKind::Audio => Media::Audio(Audio::parse(document, audio)),
            _ => Media::Document(document),
        })
    }

    pub fn kind(&self) -> MediaKind {
        match self {
            Media::Photo(_) => MediaKind::Photo,
            Media::Document(_) => MediaKind::Document,
            Media::Animation(_) => MediaKind::Animation,
            Media::Audio(_) => MediaKind::Audio,
            Media::Video(_) => MediaKind::Video,
            Media::VideoNote(_) => MediaKind::VideoNote,
            Media::Voice(_) => MediaKind::Voice,
            Media::Sticker(_) => MediaKind::Sticker,
            Media::Contact(_) => MediaKind::Contact,
            Media::Location(_) => MediaKind::Location,
            Media::Venue(_) => MediaKind::Venue,
            Media::Dice(_) => MediaKind::Dice,
            Media::Poll(_) => MediaKind::Poll,
            Media::Invoice(_) => MediaKind::Invoice,
            Media::Checklist(_) => MediaKind::Checklist,
            Media::Game(_) => MediaKind::Game,
        }
    }

    pub(crate) fn file_location(&self) -> Option<&FileLocation> {
        match self {
            Media::Photo(m) => Some(&m.location),
            Media::Document(m) => Some(&m.location),
            Media::Animation(m) => Some(&m.location),
            Media::Audio(m) => Some(&m.location),
            Media::Video(m) => Some(&m.location),
            Media::VideoNote(m) => Some(&m.location),
            Media::Voice(m) => Some(&m.location),
            Media::Sticker(m) => Some(&m.location),
            Media::Contact(_)
            | Media::Location(_)
            | Media::Venue(_)
            | Media::Dice(_)
            | Media::Poll(_)
            | Media::Invoice(_)
            | Media::Checklist(_)
            | Media::Game(_) => None,
        }
    }
}

impl Message {
    pub fn photo(&self) -> Option<&Photo> {
        if let Some(Media::Photo(m)) = &self.media { Some(m) } else { None }
    }

    pub fn document(&self) -> Option<&Document> {
        if let Some(Media::Document(m)) = &self.media { Some(m) } else { None }
    }

    pub fn animation(&self) -> Option<&Animation> {
        if let Some(Media::Animation(m)) = &self.media { Some(m) } else { None }
    }

    pub fn audio(&self) -> Option<&Audio> {
        if let Some(Media::Audio(m)) = &self.media { Some(m) } else { None }
    }

    pub fn video(&self) -> Option<&Video> {
        if let Some(Media::Video(m)) = &self.media { Some(m) } else { None }
    }

    pub fn video_note(&self) -> Option<&VideoNote> {
        if let Some(Media::VideoNote(m)) = &self.media { Some(m) } else { None }
    }

    pub fn voice(&self) -> Option<&Voice> {
        if let Some(Media::Voice(m)) = &self.media { Some(m) } else { None }
    }

    pub fn sticker(&self) -> Option<&Sticker> {
        if let Some(Media::Sticker(m)) = &self.media { Some(m) } else { None }
    }

    pub fn contact(&self) -> Option<&Contact> {
        if let Some(Media::Contact(m)) = &self.media { Some(m) } else { None }
    }

    pub fn location(&self) -> Option<&Location> {
        if let Some(Media::Location(m)) = &self.media { Some(m) } else { None }
    }

    pub fn venue(&self) -> Option<&Venue> {
        if let Some(Media::Venue(m)) = &self.media { Some(m) } else { None }
    }

    pub fn dice(&self) -> Option<&Dice> {
        if let Some(Media::Dice(m)) = &self.media { Some(m) } else { None }
    }

    pub fn poll(&self) -> Option<&Poll> {
        if let Some(Media::Poll(m)) = &self.media { Some(m) } else { None }
    }

    pub fn invoice(&self) -> Option<&Invoice> {
        if let Some(Media::Invoice(m)) = &self.media { Some(m) } else { None }
    }

    pub fn checklist(&self) -> Option<&Checklist> {
        if let Some(Media::Checklist(m)) = &self.media { Some(m) } else { None }
    }

    pub fn game(&self) -> Option<&Game> {
        if let Some(Media::Game(m)) = &self.media { Some(m) } else { None }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::Media;
    use crate::dispatcher::Entities;
    use crate::enums::MediaKind;
    use crate::raw::{base, types};

    pub(crate) fn document_media(mime_type: &str, attributes: Vec<base::DocumentAttribute>) -> base::MessageMedia {
        let document = types::Document {
            id: 1,
            access_hash: 2,
            file_reference: vec![3],
            date: 4,
            mime_type: mime_type.into(),
            size: 5,
            thumbs: None,
            video_thumbs: None,
            dc_id: 2,
            attributes,
        };
        base::MessageMedia::MessageMediaDocument(Box::new(types::MessageMediaDocument {
            nopremium: false,
            spoiler: false,
            video: false,
            round: false,
            voice: false,
            document: Some(base::Document::Document(Box::new(document))),
            alt_documents: None,
            video_cover: None,
            video_timestamp: None,
            ttl_seconds: None,
        }))
    }

    fn video(round_message: bool) -> base::DocumentAttribute {
        base::DocumentAttribute::DocumentAttributeVideo(Box::new(types::DocumentAttributeVideo {
            round_message,
            supports_streaming: true,
            nosound: false,
            duration: 7.9,
            w: 640,
            h: 480,
            preload_prefix_size: None,
            video_start_ts: None,
            video_codec: None,
        }))
    }

    fn audio(voice: bool) -> base::DocumentAttribute {
        base::DocumentAttribute::DocumentAttributeAudio(Box::new(types::DocumentAttributeAudio {
            voice,
            duration: 3,
            title: Some("Song".into()),
            performer: None,
            waveform: voice.then(|| vec![1, 2]),
        }))
    }

    fn parse(media: base::MessageMedia) -> Media {
        Media::parse(&media, &Entities::default()).unwrap()
    }

    #[test]
    fn picks_the_most_specific_document_type() {
        let animated = vec![video(false), base::DocumentAttribute::DocumentAttributeAnimated];
        let Media::Animation(gif) = parse(document_media("video/mp4", animated)) else { panic!() };
        assert_eq!((gif.width, gif.height, gif.duration), (640, 480, 7));

        let sticker = base::DocumentAttribute::DocumentAttributeSticker(Box::new(types::DocumentAttributeSticker {
            mask: false,
            alt: "🙂".into(),
            stickerset: base::InputStickerSet::InputStickerSetEmpty,
            mask_coords: None,
        }));
        let sticker = parse(document_media("video/webm", vec![sticker, video(false)]));
        let Media::Sticker(sticker) = sticker else { panic!() };
        assert_eq!((sticker.is_video, sticker.is_animated, sticker.emoji.as_deref()), (true, false, Some("🙂")));

        let Media::VideoNote(note) = parse(document_media("video/mp4", vec![video(true)])) else { panic!() };
        assert_eq!((note.length, note.duration), (640, 7));
        let Media::Video(plain) = parse(document_media("video/mp4", vec![video(false)])) else { panic!() };
        assert_eq!((plain.width, plain.supports_streaming), (640, true));

        let Media::Voice(voice) = parse(document_media("audio/ogg", vec![audio(true)])) else { panic!() };
        assert_eq!(voice.waveform, Some(vec![1, 2]));
        let Media::Audio(song) = parse(document_media("audio/mpeg", vec![audio(false)])) else { panic!() };
        assert_eq!((song.duration, song.title.as_deref()), (3, Some("Song")));

        let name = base::DocumentAttribute::DocumentAttributeFilename(Box::new(types::DocumentAttributeFilename {
            file_name: "a.pdf".into(),
        }));
        let file = parse(document_media("application/pdf", vec![name]));
        assert_eq!(file.file_location().and_then(|l| l.file_size), Some(5));
        let Media::Document(file) = file else { panic!() };
        assert_eq!((file.file_name.as_deref(), file.file_size), (Some("a.pdf"), 5));
    }

    #[test]
    fn parsed_kind_matches_the_raw_classifier() {
        let mut round = document_media("video/mp4", Vec::new());
        if let base::MessageMedia::MessageMediaDocument(d) = &mut round {
            d.round = true;
        }
        let animated = vec![video(false), base::DocumentAttribute::DocumentAttributeAnimated];
        for media in [round, document_media("video/mp4", animated), document_media("audio/ogg", vec![audio(true)])] {
            assert_eq!(Some(parse(media.clone()).kind()), MediaKind::of(&media));
        }
    }

    #[test]
    fn maps_live_locations_and_venues() {
        let point = types::GeoPoint { long: 2.5, lat: 1.5, access_hash: 0, accuracy_radius: None };
        let geo = || base::GeoPoint::GeoPoint(Box::new(point.clone()));
        let live = base::MessageMedia::MessageMediaGeoLive(Box::new(types::MessageMediaGeoLive {
            geo: geo(),
            heading: Some(90),
            period: 900,
            proximity_notification_radius: None,
        }));
        let Media::Location(live) = parse(live) else { panic!() };
        assert_eq!((live.latitude, live.longitude, live.live_period, live.heading), (1.5, 2.5, Some(900), Some(90)));

        let venue = base::MessageMedia::MessageMediaVenue(Box::new(types::MessageMediaVenue {
            geo: geo(),
            title: "Cafe".into(),
            address: "Main St".into(),
            provider: "gplaces".into(),
            venue_id: "abc".into(),
            venue_type: String::new(),
        }));
        let Media::Venue(venue) = parse(venue) else { panic!() };
        assert_eq!((venue.google_place_id.as_deref(), venue.google_place_type), (Some("abc"), None));
        assert_eq!((venue.title.as_str(), venue.foursquare_id), ("Cafe", None));
    }
}
