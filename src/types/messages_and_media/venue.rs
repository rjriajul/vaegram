use crate::raw::{base, types};
use crate::types::Location;

/// A named place on the map, such as a restaurant.
#[derive(Debug, Clone, PartialEq)]
pub struct Venue {
    /// Where the venue is.
    pub location: Location,
    /// Name of the venue.
    pub title: String,
    /// Street address of the venue.
    pub address: String,
    /// Foursquare id. `None` unless the provider is Foursquare.
    pub foursquare_id: Option<String>,
    /// Foursquare category. `None` unless the provider is Foursquare.
    pub foursquare_type: Option<String>,
    /// Google Places id. `None` unless the provider is Google Places.
    pub google_place_id: Option<String>,
    /// Google Places category. `None` unless the provider is Google Places.
    pub google_place_type: Option<String>,
}

impl Venue {
    /// Creates a venue at the given coordinates with no provider ids.
    pub fn new(latitude: f64, longitude: f64, title: impl Into<String>, address: impl Into<String>) -> Self {
        Venue {
            location: Location::new(latitude, longitude),
            title: title.into(),
            address: address.into(),
            foursquare_id: None,
            foursquare_type: None,
            google_place_id: None,
            google_place_type: None,
        }
    }

    pub(crate) fn parse(venue: &types::MessageMediaVenue) -> Option<Venue> {
        let (title, address) = (venue.title.clone(), venue.address.clone());
        Venue::from_provider(&venue.geo, title, address, &venue.provider, &venue.venue_id, &venue.venue_type)
    }

    /// Telegram keeps one place id: the provider says whether it is a Foursquare or a Google Places one.
    pub(crate) fn from_provider(
        geo: &base::GeoPoint,
        title: String,
        address: String,
        provider: &str,
        venue_id: &str,
        venue_type: &str,
    ) -> Option<Venue> {
        let some = |s: &str| (!s.is_empty()).then(|| s.to_owned());
        let mut parsed = Venue {
            location: Location::parse(geo)?,
            title,
            address,
            foursquare_id: None,
            foursquare_type: None,
            google_place_id: None,
            google_place_type: None,
        };
        let ids = (some(venue_id), some(venue_type));
        match provider {
            "foursquare" => (parsed.foursquare_id, parsed.foursquare_type) = ids,
            "gplaces" => (parsed.google_place_id, parsed.google_place_type) = ids,
            _ => {}
        }
        Some(parsed)
    }
}
