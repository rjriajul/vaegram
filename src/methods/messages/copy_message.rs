use super::send_options::{SendOptions, caption_setters, send_builder};
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::types::messages_and_media::{input_document_of, input_photo_of};
use crate::types::{Message, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct CopyMessage<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    from_chat_id: PeerRef,
    message_id: i32,
    caption: Option<String>,
    parse_mode: Option<ParseMode>,
    options: SendOptions,
}

impl Client {
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
        }
    }
}

impl CopyMessage<'_> {
    pub fn caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    async fn send(mut self) -> Result<Message> {
        let client = self.client;
        let (mut found, _) = client.get_raw_messages(self.from_chat_id, &[self.message_id]).await?;
        let source = *found.pop().flatten().ok_or(Error::Unexpected("getMessages returned no message to copy"))?;
        self.options.no_webpage = !matches!(source.media, Some(base::MessageMedia::MessageMediaWebPage(_)));
        self.options.invert_media = source.invert_media;
        let media = source.media.map(input_media).transpose()?.flatten();
        let text = match (&self.caption, media.is_some()) {
            (Some(caption), true) => {
                let (text, entities) = client.parse_text(caption, self.parse_mode).await?;
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
    mut entities: Option<Vec<base::MessageEntity>>,
) -> Result<Option<Vec<base::MessageEntity>>> {
    for entity in entities.iter_mut().flatten() {
        if let base::MessageEntity::MessageEntityMentionName(m) = entity {
            let user_id = client.resolve_user(&m.user_id.into()).await?;
            let mention = types::InputMessageEntityMentionName { offset: m.offset, length: m.length, user_id };
            *entity = base::MessageEntity::InputMessageEntityMentionName(Box::new(mention));
        }
    }
    Ok(entities)
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
