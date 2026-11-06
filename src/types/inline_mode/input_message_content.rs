use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::methods::geo_point;
use crate::raw::{base, types};
use crate::types::{InputInvoice, InputRichMessage, Text};

/// Content of the message sent when the user picks an inline result.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum InputMessageContent {
    /// A text message.
    Text {
        /// The message text. Text made straight from a string is parsed with `parse_mode`; a built or parsed
        /// [`Text`] is sent as it is.
        text: Text,
        /// How to parse `text`. `None` uses the client's default parse mode.
        parse_mode: Option<ParseMode>,
        /// Sends no link preview; `preview_url` is then ignored.
        no_webpage: bool,
        /// Shows the link preview above the text instead of below it.
        invert_media: bool,
        /// The link to preview instead of the first one in the text. If Telegram can't build its preview, the
        /// message is sent without one. `None` previews the first link.
        preview_url: Option<String>,
        /// Asks for a large preview image. Only applies with `preview_url`.
        prefer_large_media: bool,
        /// Asks for a small preview image. Only applies with `preview_url`.
        prefer_small_media: bool,
    },
    /// A location.
    Location {
        /// Latitude in degrees.
        latitude: f64,
        /// Longitude in degrees.
        longitude: f64,
        /// Radius of uncertainty in meters, 0 to 1500. `None` if unknown.
        horizontal_accuracy: Option<i32>,
        /// For a live location, how long it can be updated, in seconds: 60 to 86400, or `0x7FFFFFFF` for no end.
        /// `None` sends a static location.
        live_period: Option<i32>,
        /// For a live location, the direction the user is moving in degrees, 1 to 360. `None` if unknown.
        heading: Option<i32>,
        /// For a live location, the distance in meters, 1 to 100000, within which other members are alerted that
        /// the user is approaching. `None` for no alerts.
        proximity_alert_radius: Option<i32>,
    },
    /// A venue.
    Venue {
        /// Latitude in degrees.
        latitude: f64,
        /// Longitude in degrees.
        longitude: f64,
        /// Name of the venue.
        title: String,
        /// Address of the venue.
        address: String,
        /// `foursquare` or `gplaces`, matching `venue_id` and `venue_type`.
        provider: String,
        /// The venue's id in `provider`'s database. Empty if not set.
        venue_id: String,
        /// The venue's type in `provider`'s terms, such as a Foursquare category. Empty if not set.
        venue_type: String,
    },
    /// A contact.
    Contact {
        /// Contact's phone number.
        phone_number: String,
        /// Contact's first name.
        first_name: String,
        /// Contact's last name. Empty if not set.
        last_name: String,
        /// Extra contact data as a vCard. Empty if not set.
        vcard: String,
    },
    /// An invoice.
    Invoice(Box<InputInvoice>),
    /// A rich message.
    Rich(InputRichMessage),
}

impl InputMessageContent {
    /// Creates text content parsed with the client's default parse mode.
    pub fn text(text: impl Into<Text>) -> Self {
        InputMessageContent::formatted(text.into(), None)
    }

    /// Creates text content parsed as HTML.
    pub fn html(text: impl Into<Text>) -> Self {
        InputMessageContent::formatted(text.into(), Some(ParseMode::Html))
    }

    /// Creates text content parsed as Markdown.
    pub fn markdown(text: impl Into<Text>) -> Self {
        InputMessageContent::formatted(text.into(), Some(ParseMode::Markdown))
    }

    fn formatted(text: Text, parse_mode: Option<ParseMode>) -> Self {
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

    /// Creates content that sends a location.
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

    /// Creates venue content with the given coordinates, name and address.
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

    /// Creates content that sends a contact.
    pub fn contact(phone_number: impl Into<String>, first_name: impl Into<String>) -> Self {
        InputMessageContent::Contact {
            phone_number: phone_number.into(),
            first_name: first_name.into(),
            last_name: String::new(),
            vcard: String::new(),
        }
    }

    /// Creates content that sends an invoice.
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
                let (message, entities) = client.write_text(text, parse_mode).await?;
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
                let rich_message = rich.write(client).await?;
                let rich = types::InputBotInlineMessageRichMessage { reply_markup, rich_message };
                base::InputBotInlineMessage::InputBotInlineMessageRichMessage(Box::new(rich))
            }
        })
    }
}

impl From<String> for InputMessageContent {
    fn from(text: String) -> Self {
        InputMessageContent::formatted(text.into(), Some(ParseMode::Disabled))
    }
}

impl From<&str> for InputMessageContent {
    fn from(text: &str) -> Self {
        InputMessageContent::formatted(text.into(), Some(ParseMode::Disabled))
    }
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
