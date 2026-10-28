use crate::raw::base;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
    pub horizontal_accuracy: Option<i32>,
    pub live_period: Option<i32>,
    pub heading: Option<i32>,
    pub proximity_alert_radius: Option<i32>,
}

impl Location {
    pub fn new(latitude: f64, longitude: f64) -> Self {
        Location {
            latitude,
            longitude,
            horizontal_accuracy: None,
            live_period: None,
            heading: None,
            proximity_alert_radius: None,
        }
    }

    pub(crate) fn parse(geo: &base::GeoPoint) -> Option<Location> {
        match geo {
            base::GeoPoint::GeoPoint(g) => Some(Location {
                latitude: g.lat,
                longitude: g.long,
                horizontal_accuracy: g.accuracy_radius,
                live_period: None,
                heading: None,
                proximity_alert_radius: None,
            }),
            base::GeoPoint::GeoPointEmpty => None,
        }
    }
}
