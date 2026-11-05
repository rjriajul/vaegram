use crate::raw::base;
use crate::types::Location;

/// The address of a business account, with a map point when it set one.
#[derive(Debug, Clone, PartialEq)]
pub struct BusinessLocation {
    /// Street address as the owner wrote it.
    pub address: String,
    /// Map point of the address. `None` if the owner set none.
    pub location: Option<Location>,
}

// Telegram never sends NaN coordinates, so equality is total and chats can stay `Eq`.
impl Eq for BusinessLocation {}

impl BusinessLocation {
    pub(crate) fn parse(raw: base::BusinessLocation) -> BusinessLocation {
        let base::BusinessLocation::BusinessLocation(l) = raw;
        let l = *l;
        BusinessLocation { address: l.address, location: l.geo_point.as_ref().and_then(Location::parse) }
    }
}

#[cfg(test)]
mod tests {
    use super::BusinessLocation;
    use crate::raw::{base, types};

    #[test]
    fn reads_address_and_point() {
        let parse = |geo_point| {
            let raw = types::BusinessLocation { geo_point, address: "1 Cat Street".into() };
            BusinessLocation::parse(base::BusinessLocation::BusinessLocation(raw.into()))
        };
        let point = types::GeoPoint { long: 13.4, lat: 52.5, access_hash: 1, accuracy_radius: None };
        let with = parse(Some(base::GeoPoint::GeoPoint(point.into())));
        assert_eq!(with.address, "1 Cat Street");
        assert_eq!(with.location.map(|l| (l.latitude, l.longitude)), Some((52.5, 13.4)));
        assert_eq!(parse(Some(base::GeoPoint::GeoPointEmpty)).location, None);
        assert_eq!(parse(None).location, None);
    }
}
