use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::{InputInvoice, InputRichMessage};

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum InputMessageContent {
    Text {
        text: String,
        parse_mode: Option<ParseMode>,
        no_webpage: bool,
        invert_media: bool,
        preview_url: Option<String>,
        prefer_large_media: bool,
        prefer_small_media: bool,
    },
    Location {
        latitude: f64,
        longitude: f64,
        horizontal_accuracy: Option<i32>,
        live_period: Option<i32>,
        heading: Option<i32>,
        proximity_alert_radius: Option<i32>,
    },
    Venue {
        latitude: f64,
        longitude: f64,
        title: String,
        address: String,
        /// `foursquare` or `gplaces`, matching `venue_id` and `venue_type`.
        provider: String,
        venue_id: String,
        venue_type: String,
    },
    Contact {
        phone_number: String,
        first_name: String,
        last_name: String,
        vcard: String,
    },
    Invoice(Box<InputInvoice>),
    Rich(InputRichMessage),
}

impl InputMessageContent {
    pub fn text(text: impl Into<String>) -> Self {
        InputMessageContent::formatted(text.into(), None)
    }

    pub fn html(text: impl Into<String>) -> Self {
        InputMessageContent::formatted(text.into(), Some(ParseMode::Html))
    }

    pub fn markdown(text: impl Into<String>) -> Self {
        InputMessageContent::formatted(text.into(), Some(ParseMode::Markdown))
    }

    fn formatted(text: String, parse_mode: Option<ParseMode>) -> Self {
        InputMessageContent::Text {
            text,
            parse_mode,
            no_webpage: false,
            invert_media: false,
            preview_url: None,
            prefer_large_media: false,
            prefer_small_media: false,
        }
    }

    pub fn location(latitude: f64, longitude: f64) -> Self {
        InputMessageContent::Location {
            latitude,
            longitude,
            horizontal_accuracy: None,
            live_period: None,
            heading: None,
            proximity_alert_radius: None,
        }
    }

    pub fn venue(latitude: f64, longitude: f64, title: impl Into<String>, address: impl Into<String>) -> Self {
        InputMessageContent::Venue {
            latitude,
            longitude,
            title: title.into(),
            address: address.into(),
            provider: String::new(),
            venue_id: String::new(),
            venue_type: String::new(),
        }
    }

    pub fn contact(phone_number: impl Into<String>, first_name: impl Into<String>) -> Self {
        InputMessageContent::Contact {
            phone_number: phone_number.into(),
            first_name: first_name.into(),
            last_name: String::new(),
            vcard: String::new(),
        }
    }

    pub fn invoice(invoice: InputInvoice) -> Self {
        InputMessageContent::Invoice(Box::new(invoice))
    }

    pub(crate) async fn write(
        self,
        client: &Client,
        reply_markup: Option<base::ReplyMarkup>,
    ) -> Result<base::InputBotInlineMessage> {
        Ok(match self {
            InputMessageContent::Text {
                text,
                parse_mode,
                no_webpage,
                invert_media,
                preview_url,
                prefer_large_media,
                prefer_small_media,
            } => {
                let (message, entities) = client.parse_text(&text, parse_mode).await?;
                let entities = (!entities.is_empty()).then_some(entities);
                match preview_url.filter(|_| !no_webpage) {
                    // `optional` keeps the send from failing when Telegram can't build the preview.
                    Some(url) => base::InputBotInlineMessage::InputBotInlineMessageMediaWebPage(Box::new(
                        types::InputBotInlineMessageMediaWebPage {
                            invert_media,
                            force_large_media: prefer_large_media,
                            force_small_media: prefer_small_media,
                            optional: true,
                            message,
                            entities,
                            url,
                            reply_markup,
                        },
                    )),
                    None => base::InputBotInlineMessage::InputBotInlineMessageText(Box::new(
                        types::InputBotInlineMessageText { no_webpage, invert_media, message, entities, reply_markup },
                    )),
                }
            }
            InputMessageContent::Location {
                latitude,
                longitude,
                horizontal_accuracy,
                live_period,
                heading,
                proximity_alert_radius,
            } => base::InputBotInlineMessage::InputBotInlineMessageMediaGeo(Box::new(
                types::InputBotInlineMessageMediaGeo {
                    geo_point: geo_point(latitude, longitude, horizontal_accuracy),
                    heading,
                    period: live_period,
                    proximity_notification_radius: proximity_alert_radius,
                    reply_markup,
                },
            )),
            InputMessageContent::Venue { latitude, longitude, title, address, provider, venue_id, venue_type } => {
                base::InputBotInlineMessage::InputBotInlineMessageMediaVenue(Box::new(
                    types::InputBotInlineMessageMediaVenue {
                        geo_point: geo_point(latitude, longitude, None),
                        title,
                        address,
                        provider,
                        venue_id,
                        venue_type,
                        reply_markup,
                    },
                ))
            }
            InputMessageContent::Contact { phone_number, first_name, last_name, vcard } => {
                base::InputBotInlineMessage::InputBotInlineMessageMediaContact(Box::new(
                    types::InputBotInlineMessageMediaContact {
                        phone_number,
                        first_name,
                        last_name,
                        vcard,
                        reply_markup,
                    },
                ))
            }
            InputMessageContent::Invoice(invoice) => invoice.write_inline(reply_markup),
            InputMessageContent::Rich(rich) => {
                let rich_message = rich.write();
                let rich = types::InputBotInlineMessageRichMessage { reply_markup, rich_message };
                base::InputBotInlineMessage::InputBotInlineMessageRichMessage(Box::new(rich))
            }
        })
    }
}

impl From<String> for InputMessageContent {
    fn from(text: String) -> Self {
        InputMessageContent::text(text)
    }
}

impl From<&str> for InputMessageContent {
    fn from(text: &str) -> Self {
        InputMessageContent::text(text)
    }
}

fn geo_point(lat: f64, long: f64, accuracy_radius: Option<i32>) -> base::InputGeoPoint {
    base::InputGeoPoint::InputGeoPoint(Box::new(types::InputGeoPoint { lat, long, accuracy_radius }))
}

// Telegram downloads web documents itself, so the size is unknown up front and sent as 0.
pub(crate) fn web_document(
    url: String,
    mime_type: String,
    attributes: Vec<base::DocumentAttribute>,
) -> base::InputWebDocument {
    base::InputWebDocument::InputWebDocument(Box::new(types::InputWebDocument { url, size: 0, mime_type, attributes }))
}

pub(crate) fn web_photo(url: String) -> base::InputWebDocument {
    web_document(url, "image/jpeg".into(), Vec::new())
}
