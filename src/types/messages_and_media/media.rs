use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::{FileType, MediaKind};
use crate::raw::base;
use crate::types::{
    Animation, Audio, Checklist, Contact, Dice, Document, FileLocation, Game, Giveaway, GiveawayWinners, Invoice,
    LivePhoto, Location, Message, PaidMediaInfo, Photo, Poll, Sticker, Story, Venue, Video, VideoNote, Voice,
};

/// The media a message carries, one variant per kind.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Media {
    /// A photo.
    Photo(Photo),
    /// A file sent as a document.
    Document(Document),
    /// A GIF or a silent looping video.
    Animation(Animation),
    /// An audio file.
    Audio(Audio),
    /// A video.
    Video(Video),
    /// A round video message.
    VideoNote(VideoNote),
    /// A voice note.
    Voice(Voice),
    /// A sticker.
    Sticker(Sticker),
    /// A photo with a short video.
    LivePhoto(LivePhoto),
    /// A shared contact.
    Contact(Contact),
    /// A shared location, static or live.
    Location(Location),
    /// A shared venue.
    Venue(Venue),
    /// An animated dice, with the value it landed on.
    Dice(Dice),
    /// A poll or quiz.
    Poll(Poll),
    /// An invoice.
    Invoice(Invoice),
    /// A checklist.
    Checklist(Checklist),
    /// A game.
    Game(Game),
    /// Media behind a Stars paywall.
    PaidMedia(PaidMediaInfo),
    /// A forwarded story. Boxed, since a story holds media itself.
    Story(Box<Story>),
    /// A giveaway announcement.
    Giveaway(Giveaway),
    /// The results of a finished giveaway. Boxed, since it holds a whole `Chat`.
    GiveawayWinners(Box<GiveawayWinners>),
    /// Media of a kind this crate has no variant for, such as a live stream or a widget.
    #[non_exhaustive]
    Unknown {
        /// TL constructor id of the media, for logging.
        constructor_id: u32,
    },
}

impl Media {
    pub(crate) fn parse(client: &Client, media: &base::MessageMedia, entities: &Entities) -> Option<Media> {
        use base::MessageMedia as M;
        let kind = MediaKind::of(media)?;
        Some(match media {
            M::MessageMediaPhoto(m) if LivePhoto::is_live(m) => Media::LivePhoto(LivePhoto::parse(m)?),
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
            M::MessageMediaPaidMedia(m) => Media::PaidMedia(PaidMediaInfo::parse(client, m, entities)),
            M::MessageMediaStory(m) => {
                let item = m.story.clone().and_then(|item| Story::parse(client, &m.peer, item, entities));
                Media::Story(Box::new(item.unwrap_or_else(|| Story::from_media(client, &m.peer, m.id, entities))))
            }
            M::MessageMediaGiveaway(m) => Media::Giveaway(Giveaway::parse(m, entities)),
            M::MessageMediaGiveawayResults(m) => Media::GiveawayWinners(Box::new(GiveawayWinners::parse(m, entities))),
            // A link preview is `Message::web_page`, not media.
            M::MessageMediaWebPage(_) => return None,
            _ => Media::Unknown { constructor_id: media.constructor_id() },
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
        let file_type = match kind {
            MediaKind::Sticker => FileType::Sticker,
            MediaKind::Animation => FileType::Animation,
            MediaKind::VideoNote => FileType::VideoNote,
            MediaKind::Video => FileType::Video,
            MediaKind::Voice => FileType::Voice,
            MediaKind::Audio => FileType::Audio,
            _ => FileType::Document,
        };
        let document = Document::parse_as(document, file_type)?;
        Some(match kind {
            MediaKind::Sticker => Media::Sticker(Sticker::from_document(document, raw)?),
            MediaKind::Animation => Media::Animation(Animation::parse(document, video, size)),
            MediaKind::VideoNote => Media::VideoNote(VideoNote::parse(document, video)),
            MediaKind::Video => Media::Video(Video::parse(document, video)),
            MediaKind::Voice => Media::Voice(Voice::parse(document, audio)),
            MediaKind::Audio => Media::Audio(Audio::parse(document, audio)),
            _ => Media::Document(document),
        })
    }

    /// The [`MediaKind`] of this media, the same value message filters match on.
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
            Media::LivePhoto(_) => MediaKind::LivePhoto,
            Media::Contact(_) => MediaKind::Contact,
            Media::Location(_) => MediaKind::Location,
            Media::Venue(_) => MediaKind::Venue,
            Media::Dice(_) => MediaKind::Dice,
            Media::Poll(_) => MediaKind::Poll,
            Media::Invoice(_) => MediaKind::Invoice,
            Media::Checklist(_) => MediaKind::Checklist,
            Media::Game(_) => MediaKind::Game,
            Media::PaidMedia(_) => MediaKind::PaidMedia,
            Media::Story(_) => MediaKind::Story,
            Media::Giveaway(_) => MediaKind::Giveaway,
            Media::GiveawayWinners(_) => MediaKind::GiveawayWinners,
            Media::Unknown { .. } => MediaKind::Unknown,
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
            Media::LivePhoto(m) => Some(&m.location),
            Media::Contact(_)
            | Media::Location(_)
            | Media::Venue(_)
            | Media::Dice(_)
            | Media::Poll(_)
            | Media::Invoice(_)
            | Media::Checklist(_)
            | Media::Game(_)
            | Media::PaidMedia(_)
            | Media::Story(_)
            | Media::Giveaway(_)
            | Media::GiveawayWinners(_)
            | Media::Unknown { .. } => None,
        }
    }
}

impl Message {
    /// The photo, if the message's media is one.
    pub fn photo(&self) -> Option<&Photo> {
        if let Some(Media::Photo(m)) = &self.media { Some(m) } else { None }
    }

    /// The document, if the message's media is one.
    pub fn document(&self) -> Option<&Document> {
        if let Some(Media::Document(m)) = &self.media { Some(m) } else { None }
    }

    /// The animation, if the message's media is one.
    pub fn animation(&self) -> Option<&Animation> {
        if let Some(Media::Animation(m)) = &self.media { Some(m) } else { None }
    }

    /// The audio file, if the message's media is one.
    pub fn audio(&self) -> Option<&Audio> {
        if let Some(Media::Audio(m)) = &self.media { Some(m) } else { None }
    }

    /// The video, if the message's media is one.
    pub fn video(&self) -> Option<&Video> {
        if let Some(Media::Video(m)) = &self.media { Some(m) } else { None }
    }

    /// The video note, if the message's media is one.
    pub fn video_note(&self) -> Option<&VideoNote> {
        if let Some(Media::VideoNote(m)) = &self.media { Some(m) } else { None }
    }

    /// The voice note, if the message's media is one.
    pub fn voice(&self) -> Option<&Voice> {
        if let Some(Media::Voice(m)) = &self.media { Some(m) } else { None }
    }

    /// The sticker, if the message's media is one.
    pub fn sticker(&self) -> Option<&Sticker> {
        if let Some(Media::Sticker(m)) = &self.media { Some(m) } else { None }
    }

    /// The live photo, if the message's media is one.
    pub fn live_photo(&self) -> Option<&LivePhoto> {
        if let Some(Media::LivePhoto(m)) = &self.media { Some(m) } else { None }
    }

    /// The contact, if the message's media is one.
    pub fn contact(&self) -> Option<&Contact> {
        if let Some(Media::Contact(m)) = &self.media { Some(m) } else { None }
    }

    /// The location, if the message's media is one.
    pub fn location(&self) -> Option<&Location> {
        if let Some(Media::Location(m)) = &self.media { Some(m) } else { None }
    }

    /// The venue, if the message's media is one.
    pub fn venue(&self) -> Option<&Venue> {
        if let Some(Media::Venue(m)) = &self.media { Some(m) } else { None }
    }

    /// The dice, if the message's media is one.
    pub fn dice(&self) -> Option<&Dice> {
        if let Some(Media::Dice(m)) = &self.media { Some(m) } else { None }
    }

    /// The poll, if the message's media is one.
    pub fn poll(&self) -> Option<&Poll> {
        if let Some(Media::Poll(m)) = &self.media { Some(m) } else { None }
    }

    /// The invoice, if the message's media is one.
    pub fn invoice(&self) -> Option<&Invoice> {
        if let Some(Media::Invoice(m)) = &self.media { Some(m) } else { None }
    }

    /// The checklist, if the message's media is one.
    pub fn checklist(&self) -> Option<&Checklist> {
        if let Some(Media::Checklist(m)) = &self.media { Some(m) } else { None }
    }

    /// The game, if the message's media is one.
    pub fn game(&self) -> Option<&Game> {
        if let Some(Media::Game(m)) = &self.media { Some(m) } else { None }
    }

    /// The forwarded story, if the message's media is one.
    pub fn story(&self) -> Option<&Story> {
        if let Some(Media::Story(m)) = &self.media { Some(&**m) } else { None }
    }

    /// The giveaway, if the message's media is one.
    pub fn giveaway(&self) -> Option<&Giveaway> {
        if let Some(Media::Giveaway(m)) = &self.media { Some(m) } else { None }
    }

    /// The giveaway winners, if the message's media is one.
    pub fn giveaway_winners(&self) -> Option<&GiveawayWinners> {
        if let Some(Media::GiveawayWinners(m)) = &self.media { Some(&**m) } else { None }
    }

    /// The paid media, if the message's media is one.
    pub fn paid_media(&self) -> Option<&PaidMediaInfo> {
        if let Some(Media::PaidMedia(m)) = &self.media { Some(m) } else { None }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::Media;
    use crate::Client;
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
        Media::parse(&Client::offline(), &media, &Entities::default()).unwrap()
    }

    #[test]
    fn picks_the_most_specific_document_type() {
        let animated = vec![video(false), base::DocumentAttribute::DocumentAttributeAnimated];
        let Media::Animation(gif) = parse(document_media("video/mp4", animated)) else { panic!() };
        assert_eq!((gif.width, gif.height, gif.duration), (640, 480, 7));
        assert_eq!((gif.file_id.as_str(), gif.file_unique_id.as_str()), ("CgACAgIAAwEDAAIBAAcCAAceBA", "AgADAQAH"));

        let sticker = base::DocumentAttribute::DocumentAttributeSticker(Box::new(types::DocumentAttributeSticker {
            mask: false,
            alt: "🙂".into(),
            stickerset: base::InputStickerSet::InputStickerSetEmpty,
            mask_coords: None,
        }));
        let sticker = parse(document_media("video/webm", vec![sticker, video(false)]));
        let Media::Sticker(sticker) = sticker else { panic!() };
        assert_eq!((sticker.is_video, sticker.is_animated, sticker.emoji.as_deref()), (true, false, Some("🙂")));
        assert_eq!(sticker.file_id, "CAACAgIAAwEDAAIBAAcCAAceBA");

        let Media::VideoNote(note) = parse(document_media("video/mp4", vec![video(true)])) else { panic!() };
        assert_eq!((note.length, note.duration), (640, 7));
        assert_eq!(note.file_id, "DQACAgIAAwEDAAIBAAcCAAceBA");
        let Media::Video(plain) = parse(document_media("video/mp4", vec![video(false)])) else { panic!() };
        assert_eq!((plain.width, plain.supports_streaming), (640, true));
        assert_eq!(plain.file_id, "BAACAgIAAwEDAAIBAAcCAAceBA");

        let Media::Voice(voice) = parse(document_media("audio/ogg", vec![audio(true)])) else { panic!() };
        assert_eq!(voice.waveform, Some(vec![1, 2]));
        assert_eq!(voice.file_id, "AwACAgIAAwEDAAIBAAcCAAceBA");
        let Media::Audio(song) = parse(document_media("audio/mpeg", vec![audio(false)])) else { panic!() };
        assert_eq!((song.duration, song.title.as_deref()), (3, Some("Song")));
        assert_eq!(song.file_id, "CQACAgIAAwEDAAIBAAcCAAceBA");

        let name = base::DocumentAttribute::DocumentAttributeFilename(Box::new(types::DocumentAttributeFilename {
            file_name: "a.pdf".into(),
        }));
        let file = parse(document_media("application/pdf", vec![name]));
        assert_eq!(file.file_location().and_then(|l| l.file_size), Some(5));
        let Media::Document(file) = file else { panic!() };
        assert_eq!((file.file_name.as_deref(), file.file_size), (Some("a.pdf"), 5));
        assert_eq!(file.file_id, "BQACAgIAAwEDAAIBAAcCAAceBA");
    }

    #[test]
    fn parsed_kind_matches_the_raw_classifier() {
        let mut round = document_media("video/mp4", Vec::new());
        if let base::MessageMedia::MessageMediaDocument(d) = &mut round {
            d.round = true;
        }
        let animated = vec![video(false), base::DocumentAttribute::DocumentAttributeAnimated];
        let voice = document_media("audio/ogg", vec![audio(true)]);
        let mut inputs = vec![round, document_media("video/mp4", animated), voice];
        inputs.extend(unreadable_live_photos());
        let photo = |photo| {
            let photo =
                types::MessageMediaPhoto { spoiler: false, live_photo: false, photo, ttl_seconds: None, video: None };
            base::MessageMedia::MessageMediaPhoto(Box::new(photo))
        };
        let document = |document| {
            let media = types::MessageMediaDocument {
                nopremium: false,
                spoiler: false,
                video: false,
                round: false,
                voice: false,
                alt_documents: None,
                video_cover: None,
                video_timestamp: None,
                document,
                ttl_seconds: None,
            };
            base::MessageMedia::MessageMediaDocument(Box::new(media))
        };
        let empty_photo = base::Photo::PhotoEmpty(types::PhotoEmpty { id: 1 });
        let empty_document = base::Document::DocumentEmpty(types::DocumentEmpty { id: 1 });
        let cached = types::PhotoCachedSize { r#type: "i".into(), w: 8, h: 8, bytes: vec![1] };
        let cached_only = base::Photo::Photo(Box::new(types::Photo {
            has_stickers: false,
            id: 9,
            access_hash: 2,
            file_reference: Vec::new(),
            date: 3,
            sizes: vec![base::PhotoSize::PhotoCachedSize(Box::new(cached))],
            video_sizes: None,
            dc_id: 2,
        }));
        inputs.extend([
            photo(Some(cached_only)),
            photo(None),
            photo(Some(empty_photo)),
            document(None),
            document(Some(empty_document)),
        ]);
        for media in inputs {
            let parsed = Media::parse(&Client::offline(), &media, &Entities::default());
            assert_eq!(parsed.map(|m| m.kind()), MediaKind::of(&media));
        }
    }

    #[test]
    fn unmapped_media_is_unknown_with_its_constructor_id() {
        use crate::raw::Deserializable;
        // messageMediaUnsupported: what Telegram sends for media the client's layer can't show.
        let raw = base::MessageMedia::deserialize(&mut &0x9f84_f49e_u32.to_le_bytes()[..]).unwrap();
        let media = parse(raw.clone());
        assert!(matches!(media, Media::Unknown { constructor_id: 0x9f84_f49e }));
        assert_eq!(Some(media.kind()), MediaKind::of(&raw));
    }

    fn live_photo(video: Option<base::Document>) -> base::MessageMedia {
        let photo =
            types::MessageMediaPhoto { spoiler: false, live_photo: true, photo: None, ttl_seconds: None, video };
        base::MessageMedia::MessageMediaPhoto(Box::new(photo))
    }

    /// Live photos with photo 9 and no video or an empty one.
    fn unreadable_live_photos() -> Vec<base::MessageMedia> {
        let size = types::PhotoSize { r#type: "x".into(), w: 8, h: 8, size: 64 };
        let photo = base::Photo::Photo(Box::new(types::Photo {
            has_stickers: false,
            id: 9,
            access_hash: 2,
            file_reference: Vec::new(),
            date: 3,
            sizes: vec![base::PhotoSize::PhotoSize(Box::new(size))],
            video_sizes: None,
            dc_id: 2,
        }));
        let empty = base::Document::DocumentEmpty(types::DocumentEmpty { id: 1 });
        [None, Some(empty)]
            .into_iter()
            .map(|video| {
                let mut media = live_photo(video);
                if let base::MessageMedia::MessageMediaPhoto(m) = &mut media {
                    m.photo = Some(photo.clone());
                }
                media
            })
            .collect()
    }

    #[test]
    fn reads_the_video_of_a_live_photo() {
        let base::MessageMedia::MessageMediaDocument(document) = document_media("video/mp4", vec![video(false)]) else {
            panic!()
        };
        let media = live_photo(document.document);
        assert_eq!(Some(parse(media.clone()).kind()), MediaKind::of(&media));
        let Media::LivePhoto(live) = parse(media) else { panic!() };
        assert_eq!((live.id, live.width, live.height, live.duration, live.file_size), (1, 640, 480, 7, 5));
        assert_eq!((live.mime_type.as_str(), live.location.dc_id), ("video/mp4", Some(2)));
        assert_eq!(live.file_id, "BAACAgIAAwEDAAIBAAcCAAceBA");

        for media in unreadable_live_photos() {
            let Some(Media::Photo(still)) = Media::parse(&Client::offline(), &media, &Entities::default()) else {
                panic!()
            };
            assert_eq!(still.id, 9);
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

    fn story(story: Option<base::StoryItem>) -> base::MessageMedia {
        let peer = crate::utils::get_peer(7);
        base::MessageMedia::MessageMediaStory(Box::new(types::MessageMediaStory {
            via_mention: false,
            peer,
            id: 4,
            story,
        }))
    }

    #[test]
    fn stories_and_giveaways_match_the_raw_classifier() {
        let giveaway = base::MessageMedia::MessageMediaGiveaway(Box::new(super::super::giveaway::tests::giveaway()));
        let results = super::super::giveaway_winners::tests::results();
        let results = base::MessageMedia::MessageMediaGiveawayResults(Box::new(results));
        for media in [story(None), giveaway.clone(), results.clone()] {
            let parsed = parse(media.clone());
            assert_eq!(Some(parsed.kind()), MediaKind::of(&media));
            assert!(parsed.file_location().is_none());
        }
        assert!(matches!(parse(giveaway), Media::Giveaway(g) if g.prize_star_count == Some(500)));
        assert_eq!(MediaKind::of(&results), Some(MediaKind::GiveawayWinners));
        assert!(matches!(parse(results), Media::GiveawayWinners(w) if w.was_refunded));
    }

    #[test]
    fn a_story_without_its_item_keeps_chat_and_id() {
        let deleted = base::StoryItem::StoryItemDeleted(types::StoryItemDeleted { id: 4 });
        for media in [story(None), story(Some(deleted))] {
            let Media::Story(s) = parse(media) else { panic!() };
            assert_eq!((s.chat.id, s.id, s.date, s.expire_date), (7, 4, 0, 0));
            assert!(s.media.is_none() && s.caption.is_none() && !s.is_pinned);
        }
        let skipped = types::StoryItemSkipped { close_friends: true, live: false, id: 4, date: 10, expire_date: 20 };
        let skipped = base::StoryItem::StoryItemSkipped(Box::new(skipped));
        let Media::Story(s) = parse(story(Some(skipped))) else { panic!() };
        assert_eq!((s.id, s.date, s.expire_date, s.is_close_friends), (4, 10, 20, true));
    }
}
