use crate::Client;
use crate::crypto::random;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::{Message, ReplyMarkup};

pub struct CopyMessage<'a> {
    client: &'a Client,
    chat_id: i64,
    from_chat_id: i64,
    message_id: i32,
    caption: Option<String>,
    parse_mode: ParseMode,
    reply_markup: Option<ReplyMarkup>,
}

impl Client {
    pub fn copy_message(&self, chat_id: i64, from_chat_id: i64, message_id: i32) -> CopyMessage<'_> {
        CopyMessage {
            client: self,
            chat_id,
            from_chat_id,
            message_id,
            caption: None,
            parse_mode: ParseMode::Disabled,
            reply_markup: None,
        }
    }
}

impl CopyMessage<'_> {
    pub fn caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    pub fn html(self) -> Self {
        self.parse_mode(ParseMode::Html)
    }

    pub fn markdown(self) -> Self {
        self.parse_mode(ParseMode::Markdown)
    }

    pub fn parse_mode(mut self, mode: ParseMode) -> Self {
        self.parse_mode = mode;
        self
    }

    pub fn reply_markup(mut self, markup: impl Into<ReplyMarkup>) -> Self {
        self.reply_markup = Some(markup.into());
        self
    }

    async fn send(self) -> Result<Message> {
        let client = self.client;
        let (mut found, _) = client.get_raw_messages(self.from_chat_id, &[self.message_id]).await?;
        let source = *found.pop().flatten().ok_or(Error::Unexpected("getMessages returned no message to copy"))?;
        let preview = matches!(source.media, Some(base::MessageMedia::MessageMediaWebPage(_)));
        let media = source.media.map(input_media).transpose()?.flatten();
        let (message, entities) = match (&self.caption, media.is_some()) {
            (Some(caption), true) => {
                let (text, entities) = client.parse_text(caption, self.parse_mode).await?;
                (text, (!entities.is_empty()).then_some(entities))
            }
            _ => (source.message, input_entities(client, source.entities).await?),
        };
        let peer = client.resolve_peer(self.chat_id).await?;
        let random_id = i64::from_le_bytes(random());
        let reply_markup = self.reply_markup.as_ref().map(ReplyMarkup::write);
        let (updates, sent) = match media {
            Some(media) => {
                let request = functions::messages::SendMedia {
                    silent: false,
                    background: false,
                    clear_draft: false,
                    noforwards: false,
                    update_stickersets_order: false,
                    invert_media: source.invert_media,
                    allow_paid_floodskip: false,
                    peer,
                    reply_to: None,
                    media,
                    message,
                    random_id,
                    reply_markup,
                    entities,
                    schedule_date: None,
                    schedule_repeat_period: None,
                    send_as: None,
                    quick_reply_shortcut: None,
                    effect: None,
                    allow_paid_stars: None,
                    suggested_post: None,
                };
                (client.invoke(&request).await?, (request.message, request.entities))
            }
            None => {
                let request = functions::messages::SendMessage {
                    no_webpage: !preview,
                    silent: false,
                    background: false,
                    clear_draft: false,
                    noforwards: false,
                    update_stickersets_order: false,
                    invert_media: source.invert_media,
                    allow_paid_floodskip: false,
                    peer,
                    reply_to: None,
                    message,
                    random_id,
                    reply_markup,
                    entities,
                    schedule_date: None,
                    schedule_repeat_period: None,
                    send_as: None,
                    quick_reply_shortcut: None,
                    effect: None,
                    allow_paid_stars: None,
                    suggested_post: None,
                    rich_message: None,
                };
                (client.invoke(&request).await?, (request.message, request.entities))
            }
        };
        Message::from_updates(client, updates, self.chat_id, sent)
    }
}

/// Received mentions carry a bare user id; sending them needs the user's access hash.
async fn input_entities(
    client: &Client,
    mut entities: Option<Vec<base::MessageEntity>>,
) -> Result<Option<Vec<base::MessageEntity>>> {
    for entity in entities.iter_mut().flatten() {
        if let base::MessageEntity::MessageEntityMentionName(m) = entity {
            let user_id = client.resolve_user(m.user_id).await?;
            let mention = types::InputMessageEntityMentionName { offset: m.offset, length: m.length, user_id };
            *entity = base::MessageEntity::InputMessageEntityMentionName(Box::new(mention));
        }
    }
    Ok(entities)
}

fn input_media(media: base::MessageMedia) -> Result<Option<base::InputMedia>> {
    use base::MessageMedia as M;
    let unsupported = Error::Unexpected("getMessages returned media that copy_message can't resend");
    let media = match media {
        M::MessageMediaEmpty | M::MessageMediaWebPage(_) => return Ok(None),
        M::MessageMediaPhoto(m) => {
            let id = m.photo.and_then(input_photo).ok_or(unsupported)?;
            let video = m.video.and_then(input_document);
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
                id: m.document.and_then(input_document).ok_or(unsupported)?,
                video_cover: m.video_cover.and_then(input_photo),
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

fn input_photo(photo: base::Photo) -> Option<base::InputPhoto> {
    match photo {
        base::Photo::Photo(p) => {
            let photo = types::InputPhoto { id: p.id, access_hash: p.access_hash, file_reference: p.file_reference };
            Some(base::InputPhoto::InputPhoto(Box::new(photo)))
        }
        base::Photo::PhotoEmpty(_) => None,
    }
}

fn input_document(document: base::Document) -> Option<base::InputDocument> {
    match document {
        base::Document::Document(d) => {
            let document =
                types::InputDocument { id: d.id, access_hash: d.access_hash, file_reference: d.file_reference };
            Some(base::InputDocument::InputDocument(Box::new(document)))
        }
        base::Document::DocumentEmpty(_) => None,
    }
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

into_future!(CopyMessage, Message);
