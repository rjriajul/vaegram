use crate::raw::base;
use crate::types::Location;

/// Where a location-based group is, with its address.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatLocation {
    /// The point on the map.
    pub location: Location,
    /// The address set for the group's location.
    pub address: String,
}

// Telegram never sends NaN coordinates, so equality is total and chats can stay `Eq`.
impl Eq for ChatLocation {}

impl ChatLocation {
    pub(crate) fn parse(raw: base::ChannelLocation) -> Option<ChatLocation> {
        let base::ChannelLocation::ChannelLocation(l) = raw else { return None };
        let l = *l;
        Some(ChatLocation { location: Location::parse(&l.geo_point)?, address: l.address })
    }
}

#[cfg(test)]
mod tests {
    use super::ChatLocation;
    use crate::raw::{base, types};

    #[test]
    fn reads_point_and_address() {
        let point = types::GeoPoint { long: 13.4, lat: 52.5, access_hash: 1, accuracy_radius: None };
        let raw = types::ChannelLocation {
            geo_point: base::GeoPoint::GeoPoint(point.into()),
            address: "1 Cat Street".into(),
        };
        let parsed = ChatLocation::parse(base::ChannelLocation::ChannelLocation(raw.into())).unwrap();
        assert_eq!(parsed.address, "1 Cat Street");
        assert_eq!((parsed.location.latitude, parsed.location.longitude), (52.5, 13.4));
        assert_eq!(ChatLocation::parse(base::ChannelLocation::ChannelLocationEmpty), None);
    }
}
