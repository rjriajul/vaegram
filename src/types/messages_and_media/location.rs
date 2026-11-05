use crate::raw::base;

/// A point on the map.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Location {
    /// Latitude in degrees.
    pub latitude: f64,
    /// Longitude in degrees.
    pub longitude: f64,
    /// Radius of uncertainty, in meters. `None` if Telegram sent none.
    pub horizontal_accuracy: Option<i32>,
    /// How long the location can be updated, in seconds. `None` for a static location.
    pub live_period: Option<i32>,
    /// Direction the user is moving, in degrees. `None` if not given or not a live location.
    pub heading: Option<i32>,
    /// Distance for proximity alerts, in meters. `None` if not given or not a live location.
    pub proximity_alert_radius: Option<i32>,
}

impl Location {
    /// Creates a static location with no accuracy radius.
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
