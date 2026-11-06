use crate::types::InputMessageContent;

use super::inline_query_result::{Parts, setters};

/// A location on a map.
///
/// When picked, sends the location unless [`input_message_content`](Self::input_message_content) is set.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultLocation {
    parts: Parts,
    latitude: f64,
    longitude: f64,
    horizontal_accuracy: Option<i32>,
    live_period: Option<i32>,
    heading: Option<i32>,
    proximity_alert_radius: Option<i32>,
}

impl InlineQueryResultLocation {
    /// Creates a location result titled `title`.
    pub fn new(latitude: f64, longitude: f64, title: impl Into<String>) -> Self {
        InlineQueryResultLocation {
            parts: Parts { title: Some(title.into()), ..Parts::new() },
            latitude,
            longitude,
            horizontal_accuracy: None,
            live_period: None,
            heading: None,
            proximity_alert_radius: None,
        }
    }

    /// Radius of uncertainty of the location, in meters.
    #[must_use]
    pub fn horizontal_accuracy(mut self, meters: i32) -> Self {
        self.horizontal_accuracy = Some(meters);
        self
    }

    /// Makes the sent location live for this many seconds.
    #[must_use]
    pub fn live_period(mut self, seconds: i32) -> Self {
        self.live_period = Some(seconds);
        self
    }

    /// Direction the user is moving, in degrees.
    #[must_use]
    pub fn heading(mut self, degrees: i32) -> Self {
        self.heading = Some(degrees);
        self
    }

    /// Distance in meters at which a live location triggers a proximity alert.
    #[must_use]
    pub fn proximity_alert_radius(mut self, meters: i32) -> Self {
        self.proximity_alert_radius = Some(meters);
        self
    }

    pub(super) fn parts(self) -> Parts {
        let location = InputMessageContent::Location {
            latitude: self.latitude,
            longitude: self.longitude,
            horizontal_accuracy: self.horizontal_accuracy,
            live_period: self.live_period,
            heading: self.heading,
            proximity_alert_radius: self.proximity_alert_radius,
        };
        Parts { content: Some(self.parts.content.unwrap_or(location)), ..self.parts }
    }
}

setters!(InlineQueryResultLocation: id, reply_markup, input_message_content, thumbnail);
