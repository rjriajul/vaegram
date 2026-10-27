use super::send_location::geo_point;
use super::send_options::{SendOptions, send_builder};
use crate::Client;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::Message;

#[must_use = "a request does nothing until it is awaited"]
pub struct SendVenue<'a> {
    client: &'a Client,
    chat_id: i64,
    media: types::InputMediaVenue,
    options: SendOptions,
}

impl Client {
    pub fn send_venue(
        &self,
        chat_id: i64,
        latitude: f64,
        longitude: f64,
        title: impl Into<String>,
        address: impl Into<String>,
    ) -> SendVenue<'_> {
        let media = types::InputMediaVenue {
            geo_point: geo_point(latitude, longitude),
            title: title.into(),
            address: address.into(),
            provider: String::new(),
            venue_id: String::new(),
            venue_type: String::new(),
        };
        SendVenue { client: self, chat_id, media, options: SendOptions::default() }
    }
}

impl SendVenue<'_> {
    pub fn foursquare_id(mut self, id: impl Into<String>) -> Self {
        self.media.provider = "foursquare".into();
        self.media.venue_id = id.into();
        self
    }

    pub fn foursquare_type(mut self, venue_type: impl Into<String>) -> Self {
        self.media.venue_type = venue_type.into();
        self
    }

    pub fn google_place_id(mut self, id: impl Into<String>) -> Self {
        self.media.provider = "gplaces".into();
        self.media.venue_id = id.into();
        self
    }

    pub fn google_place_type(mut self, place_type: impl Into<String>) -> Self {
        self.media.venue_type = place_type.into();
        self
    }

    async fn send(self) -> Result<Message> {
        let media = base::InputMedia::InputMediaVenue(Box::new(self.media));
        self.client.send_media(self.chat_id, media, "", None, self.options).await
    }
}

send_builder!(SendVenue);

#[cfg(test)]
mod tests {
    use crate::Client;

    #[test]
    fn foursquare_id_sets_provider() {
        let client = Client::offline();
        let send = client.send_venue(1, 1.0, 2.0, "Cafe", "Main St").foursquare_id("4b5").foursquare_type("food/cafe");
        let m = &send.media;
        assert_eq!(
            (m.title.as_str(), m.address.as_str(), m.provider.as_str(), m.venue_id.as_str(), m.venue_type.as_str()),
            ("Cafe", "Main St", "foursquare", "4b5", "food/cafe")
        );
    }
}
