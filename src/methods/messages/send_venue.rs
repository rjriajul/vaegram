use super::send_location::geo_point;
use super::send_options::{SendOptions, send_builder};
use crate::Client;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::{Location, Message, PeerRef, Venue};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendVenue<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    venue: Venue,
    options: SendOptions,
}

impl Client {
    pub fn send_venue(
        &self,
        chat_id: impl Into<PeerRef>,
        latitude: f64,
        longitude: f64,
        title: impl Into<String>,
        address: impl Into<String>,
    ) -> SendVenue<'_> {
        let venue = Venue::new(latitude, longitude, title, address);
        SendVenue { client: self, chat_id: chat_id.into(), venue, options: SendOptions::default() }
    }
}

impl SendVenue<'_> {
    pub fn foursquare_id(mut self, id: impl Into<String>) -> Self {
        self.venue.foursquare_id = Some(id.into());
        self
    }

    pub fn foursquare_type(mut self, venue_type: impl Into<String>) -> Self {
        self.venue.foursquare_type = Some(venue_type.into());
        self
    }

    pub fn google_place_id(mut self, id: impl Into<String>) -> Self {
        self.venue.google_place_id = Some(id.into());
        self
    }

    pub fn google_place_type(mut self, place_type: impl Into<String>) -> Self {
        self.venue.google_place_type = Some(place_type.into());
        self
    }

    async fn send(self) -> Result<Message> {
        let media = base::InputMedia::InputMediaVenue(Box::new(input_venue(self.venue)));
        self.client.send_media(self.chat_id, media, "", None, self.options).await
    }
}

send_builder!(SendVenue);

/// Telegram keeps one place id; a Foursquare id wins over a Google Places one, as `Venue::parse` reads it back.
pub(crate) fn input_venue(venue: Venue) -> types::InputMediaVenue {
    let Venue { location: Location { latitude, longitude, .. }, title, address, .. } = venue;
    let (provider, venue_id, venue_type) = match (venue.foursquare_id, venue.google_place_id) {
        (Some(id), _) => ("foursquare", id, venue.foursquare_type),
        (None, Some(id)) => ("gplaces", id, venue.google_place_type),
        (None, None) => ("", String::new(), None),
    };
    types::InputMediaVenue {
        geo_point: geo_point(latitude, longitude, None),
        title,
        address,
        provider: provider.into(),
        venue_id,
        venue_type: venue_type.unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::input_venue;
    use crate::Client;

    #[test]
    fn foursquare_id_sets_provider() {
        let client = Client::offline();
        let send = client.send_venue(1, 1.0, 2.0, "Cafe", "Main St").foursquare_id("4b5").foursquare_type("food/cafe");
        let m = input_venue(send.venue);
        assert_eq!(
            (m.title.as_str(), m.address.as_str(), m.provider.as_str(), m.venue_id.as_str(), m.venue_type.as_str()),
            ("Cafe", "Main St", "foursquare", "4b5", "food/cafe")
        );
    }
}
