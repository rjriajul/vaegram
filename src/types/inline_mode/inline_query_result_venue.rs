use crate::types::InputMessageContent;

use super::inline_query_result::{Parts, setters};

#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultVenue {
    parts: Parts,
    latitude: f64,
    longitude: f64,
    address: String,
    provider: &'static str,
    venue_id: String,
    venue_type: String,
}

impl InlineQueryResultVenue {
    pub fn new(latitude: f64, longitude: f64, title: impl Into<String>, address: impl Into<String>) -> Self {
        InlineQueryResultVenue {
            parts: Parts { title: Some(title.into()), ..Parts::new() },
            latitude,
            longitude,
            address: address.into(),
            provider: "",
            venue_id: String::new(),
            venue_type: String::new(),
        }
    }

    pub fn foursquare(mut self, foursquare_id: impl Into<String>, foursquare_type: impl Into<String>) -> Self {
        (self.provider, self.venue_id, self.venue_type) = ("foursquare", foursquare_id.into(), foursquare_type.into());
        self
    }

    pub fn google_place(mut self, google_place_id: impl Into<String>, google_place_type: impl Into<String>) -> Self {
        (self.provider, self.venue_id, self.venue_type) = ("gplaces", google_place_id.into(), google_place_type.into());
        self
    }

    pub(super) fn parts(self) -> Parts {
        let venue = InputMessageContent::Venue {
            latitude: self.latitude,
            longitude: self.longitude,
            title: self.parts.title.clone().unwrap_or_default(),
            address: self.address,
            provider: self.provider.into(),
            venue_id: self.venue_id,
            venue_type: self.venue_type,
        };
        Parts { content: Some(self.parts.content.unwrap_or(venue)), ..self.parts }
    }
}

setters!(InlineQueryResultVenue: id, reply_markup, input_message_content, thumbnail);
