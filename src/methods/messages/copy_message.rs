use super::send_options::{SendOptions, caption_setters, send_builder};
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::types::messages_and_media::{input_document_of, input_photo_of};
use crate::types::{Message, PeerRef, Text};

#[must_use = "a request does nothing until it is awaited"]
/// Request built by [`Client::copy_message`]. Awaiting it returns the new [`Message`].
pub struct CopyMessage<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    from_chat_id: PeerRef,
    message_id: i32,
    caption: Option<Text>,
    parse_mode: Option<ParseMode>,
    options: SendOptions,
    // The message to copy when the caller already holds it and it can't be fetched by id.
    held: Option<Box<types::Message>>,
    video_start_timestamp: Option<i32>,
}

impl Client {
    /// Copies a message to `chat_id` without a link to the original.
    ///
    /// Returns the copy as a [`Message`].
    pub fn copy_message(
        &self,
        chat_id: impl Into<PeerRef>,
        from_chat_id: impl Into<PeerRef>,
        message_id: i32,
    ) -> CopyMessage<'_> {
        CopyMessage {
            client: self,
            chat_id: chat_id.into(),
            from_chat_id: from_chat_id.into(),
            message_id,
            caption: None,
            parse_mode: None,
            options: SendOptions::default(),
            held: None,
            video_start_timestamp: None,
        }
    }
}

impl CopyMessage<'_> {
    /// Replaces the caption of the copied media. Ignored when the source has no media.
    pub fn caption(mut self, caption: impl Into<Text>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// Copies `message` as it is instead of fetching `message_id` from `from_chat_id`.
    pub(crate) fn held(mut self, message: Box<types::Message>) -> Self {
        self.held = Some(message);
        self
    }

    /// Starts the copied video playing at this many seconds (the media's `video_timestamp`), replacing the start
    /// time of the source. Ignored when the source is not a video.
    pub fn video_start_timestamp(mut self, seconds: i32) -> Self {
        self.video_start_timestamp = Some(seconds);
        self
    }

    async fn send(mut self) -> Result<Message> {
        let client = self.client;
        let source = match self.held.take() {
            Some(held) => *held,
            None => {
                let (mut found, _) = client.get_raw_messages(self.from_chat_id, &[self.message_id]).await?;
                *found.pop().flatten().ok_or(Error::Unexpected("getMessages returned no message to copy"))?
            }
        };
        self.options.no_webpage = !matches!(source.media, Some(base::MessageMedia::MessageMediaWebPage(_)));
        self.options.invert_media = source.invert_media;
        let is_video = source.media.as_ref().is_some_and(is_video);
        let mut media = source.media.map(input_media).transpose()?.flatten();
        if let (true, Some(seconds), Some(base::InputMedia::InputMediaDocument(document))) =
            (is_video, self.video_start_timestamp, media.as_mut())
        {
            document.video_timestamp = Some(seconds);
        }
        let text = match (self.caption.take(), media.is_some()) {
            (Some(caption), true) => {
                let (text, entities) = client.write_text(caption, self.parse_mode).await?;
                (text, (!entities.is_empty()).then_some(entities))
            }
            _ => (source.message, input_entities(client, source.entities).await?),
        };
        client.send_parsed(self.chat_id, media, text, None, self.options).await
    }
}

caption_setters!(CopyMessage);
send_builder!(CopyMessage);

/// Received mentions carry a bare user id; sending them needs the user's access hash.
pub(super) async fn input_entities(
    client: &Client,
    entities: Option<Vec<base::MessageEntity>>,
) -> Result<Option<Vec<base::MessageEntity>>> {
    match entities {
        Some(entities) => Ok(Some(client.resolve_mentions(entities).await?)),
        None => Ok(None),
    }
}

pub(super) fn input_media(media: base::MessageMedia) -> Result<Option<base::InputMedia>> {
    use base::MessageMedia as M;
    let unsupported = Error::Unexpected("getMessages returned media that can't be copied");
    let media = match media {
        M::MessageMediaEmpty | M::MessageMediaWebPage(_) => return Ok(None),
        M::MessageMediaPhoto(m) => {
            let id = m.photo.and_then(input_photo_of).ok_or(unsupported)?;
            let video = m.video.and_then(input_document_of);
            let photo = types::InputMediaPhoto {
                spoiler: m.spoiler,
                live_photo: video.is_some(),
                id,
                ttl_seconds: m.ttl_seconds,
                video,
            };
            base::InputMedia::InputMediaPhoto(Box::new(photo))
        }
        M::MessageMediaDocument(m) => {
            let document = types::InputMediaDocument {
                spoiler: m.spoiler,
                id: m.document.and_then(input_document_of).ok_or(unsupported)?,
                video_cover: m.video_cover.and_then(input_photo_of),
                video_timestamp: m.video_timestamp,
                ttl_seconds: m.ttl_seconds,
                query: None,
            };
            base::InputMedia::InputMediaDocument(Box::new(document))
        }
        M::MessageMediaGeo(m) => {
            let geo_point = input_geo_point(m.geo).ok_or(unsupported)?;
            base::InputMedia::InputMediaGeoPoint(Box::new(types::InputMediaGeoPoint { geo_point }))
        }
        M::MessageMediaVenue(m) => {
            let venue = types::InputMediaVenue {
                geo_point: input_geo_point(m.geo).ok_or(unsupported)?,
                title: m.title,
                address: m.address,
                provider: m.provider,
                venue_id: m.venue_id,
                venue_type: m.venue_type,
            };
            base::InputMedia::InputMediaVenue(Box::new(venue))
        }
        M::MessageMediaContact(m) => {
            let contact = types::InputMediaContact {
                phone_number: m.phone_number,
                first_name: m.first_name,
                last_name: m.last_name,
                vcard: m.vcard,
            };
            base::InputMedia::InputMediaContact(Box::new(contact))
        }
        M::MessageMediaDice(m) => {
            base::InputMedia::InputMediaDice(Box::new(types::InputMediaDice { emoticon: m.emoticon }))
        }
        _ => return Err(unsupported),
    };
    Ok(Some(media))
}

fn input_geo_point(geo: base::GeoPoint) -> Option<base::InputGeoPoint> {
    match geo {
        base::GeoPoint::GeoPoint(g) => {
            let point = types::InputGeoPoint { lat: g.lat, long: g.long, accuracy_radius: g.accuracy_radius };
            Some(base::InputGeoPoint::InputGeoPoint(Box::new(point)))
        }
        base::GeoPoint::GeoPointEmpty => None,
    }
}

/// Whether `media` is a video: Telegram flags it, or the document carries video attributes.
fn is_video(media: &base::MessageMedia) -> bool {
    let base::MessageMedia::MessageMediaDocument(m) = media else { return false };
    m.video
        || matches!(&m.document, Some(base::Document::Document(d))
            if d.attributes.iter().any(|a| matches!(a, base::DocumentAttribute::DocumentAttributeVideo(_))))
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, types};
    use crate::types::Message;
    use crate::utils::get_peer;

    /// A messages.getMessages answer holding message 3 of chat -6: a video that starts at 5 seconds, or a PDF.
    fn found(video: bool) -> Vec<u8> {
        let document = types::Document {
            id: 1,
            access_hash: 2,
            file_reference: vec![3],
            date: 4,
            mime_type: if video { "video/mp4" } else { "application/pdf" }.into(),
            size: 5,
            thumbs: None,
            video_thumbs: None,
            dc_id: 2,
            attributes: Vec::new(),
        };
        let media = types::MessageMediaDocument {
            nopremium: false,
            spoiler: false,
            video,
            round: false,
            voice: false,
            document: Some(base::Document::Document(Box::new(document))),
            alt_documents: None,
            video_cover: None,
            video_timestamp: video.then_some(5),
            ttl_seconds: None,
        };
        let mut source = Message::bare(3, get_peer(-6), 0);
        source.media = Some(base::MessageMedia::MessageMediaDocument(Box::new(media)));
        let found = types::messages::Messages {
            messages: vec![base::Message::Message(Box::new(source))],
            topics: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        base::messages::Messages::Messages(Box::new(found)).to_bytes()
    }

    /// Copies message 3 with a start time of 15 seconds and returns the sendMedia body.
    async fn copied(video: bool) -> Vec<u8> {
        let (client, mut rx) = Client::scripted(vec![Ok(found(video))]);
        let _ = client.copy_message(-5, -6, 3).video_start_timestamp(15).await;
        rx.try_recv().expect("no getMessages sent");
        rx.try_recv().expect("no sendMedia sent")
    }

    fn input_document(video_timestamp: Option<i32>) -> Vec<u8> {
        let id = types::InputDocument { id: 1, access_hash: 2, file_reference: vec![3] };
        let media = types::InputMediaDocument {
            spoiler: false,
            id: base::InputDocument::InputDocument(Box::new(id)),
            video_cover: None,
            video_timestamp,
            ttl_seconds: None,
            query: None,
        };
        base::InputMedia::InputMediaDocument(Box::new(media)).to_bytes()
    }

    #[tokio::test]
    async fn the_video_start_timestamp_replaces_the_source_one() {
        let (body, expected) = (copied(true).await, input_document(Some(15)));
        assert!(body.windows(expected.len()).any(|w| w == expected), "the copy kept the source start time");
    }

    #[tokio::test]
    async fn the_video_start_timestamp_skips_other_documents() {
        let (body, expected) = (copied(false).await, input_document(None));
        assert!(body.windows(expected.len()).any(|w| w == expected), "a non-video got a start time");
    }
}
