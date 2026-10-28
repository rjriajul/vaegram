use crate::raw::types;
use crate::types::Location;

#[derive(Debug, Clone, PartialEq)]
pub struct Venue {
    pub location: Location,
    pub title: String,
    pub address: String,
    pub foursquare_id: Option<String>,
    pub foursquare_type: Option<String>,
    pub google_place_id: Option<String>,
    pub google_place_type: Option<String>,
}

impl Venue {
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
        let some = |s: &String| (!s.is_empty()).then(|| s.clone());
        let mut parsed = Venue {
            location: Location::parse(&venue.geo)?,
            title: venue.title.clone(),
            address: venue.address.clone(),
            foursquare_id: None,
            foursquare_type: None,
            google_place_id: None,
            google_place_type: None,
        };
        let ids = (some(&venue.venue_id), some(&venue.venue_type));
        match venue.provider.as_str() {
            "foursquare" => (parsed.foursquare_id, parsed.foursquare_type) = ids,
            "gplaces" => (parsed.google_place_id, parsed.google_place_type) = ids,
            _ => {}
        }
        Some(parsed)
    }
}
