use super::send_options::{SendOptions, send_builder};
use crate::Client;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::{Message, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendLocation<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    point: types::InputGeoPoint,
    live_period: Option<i32>,
    heading: Option<i32>,
    proximity_alert_radius: Option<i32>,
    options: SendOptions,
}

impl Client {
    pub fn send_location(&self, chat_id: impl Into<PeerRef>, latitude: f64, longitude: f64) -> SendLocation<'_> {
        SendLocation {
            client: self,
            chat_id: chat_id.into(),
            point: types::InputGeoPoint { lat: latitude, long: longitude, accuracy_radius: None },
            live_period: None,
            heading: None,
            proximity_alert_radius: None,
            options: SendOptions::default(),
        }
    }
}

impl SendLocation<'_> {
    pub fn horizontal_accuracy(mut self, meters: i32) -> Self {
        self.point.accuracy_radius = Some(meters);
        self
    }

    /// Seconds the location stays live; `i32::MAX` keeps it live until stopped.
    pub fn live_period(mut self, seconds: i32) -> Self {
        self.live_period = Some(seconds);
        self
    }

    /// Live locations only, like `proximity_alert_radius`.
    pub fn heading(mut self, degrees: i32) -> Self {
        self.heading = Some(degrees);
        self
    }

    pub fn proximity_alert_radius(mut self, meters: i32) -> Self {
        self.proximity_alert_radius = Some(meters);
        self
    }

    fn media(&self) -> base::InputMedia {
        let geo_point = base::InputGeoPoint::InputGeoPoint(Box::new(self.point.clone()));
        match self.live_period {
            Some(period) => base::InputMedia::InputMediaGeoLive(Box::new(types::InputMediaGeoLive {
                stopped: false,
                geo_point,
                heading: self.heading,
                period: Some(period),
                proximity_notification_radius: self.proximity_alert_radius,
            })),
            None => base::InputMedia::InputMediaGeoPoint(Box::new(types::InputMediaGeoPoint { geo_point })),
        }
    }

    async fn send(self) -> Result<Message> {
        let media = self.media();
        self.client.send_media(self.chat_id, media, "", None, self.options).await
    }
}

send_builder!(SendLocation);

pub(crate) fn geo_point(lat: f64, long: f64) -> base::InputGeoPoint {
    base::InputGeoPoint::InputGeoPoint(Box::new(types::InputGeoPoint { lat, long, accuracy_radius: None }))
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{base, types};

    #[test]
    fn maps_coordinates() {
        let client = Client::offline();
        let send = client.send_location(-100123, 51.5, -0.12).horizontal_accuracy(25).disable_notification();
        let expected = types::InputGeoPoint { lat: 51.5, long: -0.12, accuracy_radius: Some(25) };
        assert_eq!(send.point, expected);
        assert!(send.options.disable_notification);
        assert!(matches!(send.media(), base::InputMedia::InputMediaGeoPoint(_)));
    }

    #[test]
    fn live_period_sends_a_live_location() {
        let client = Client::offline();
        let send = client.send_location(1, 0.0, 0.0).live_period(900).heading(90).proximity_alert_radius(100);
        let geo_point = base::InputGeoPoint::InputGeoPoint(Box::new(send.point.clone()));
        let expected = types::InputMediaGeoLive {
            stopped: false,
            geo_point,
            heading: Some(90),
            period: Some(900),
            proximity_notification_radius: Some(100),
        };
        assert_eq!(send.media(), base::InputMedia::InputMediaGeoLive(Box::new(expected)));
    }
}
