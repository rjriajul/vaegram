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
    options: SendOptions,
}

impl Client {
    pub fn send_location(&self, chat_id: impl Into<PeerRef>, latitude: f64, longitude: f64) -> SendLocation<'_> {
        SendLocation {
            client: self,
            chat_id: chat_id.into(),
            point: types::InputGeoPoint { lat: latitude, long: longitude, accuracy_radius: None },
            options: SendOptions::default(),
        }
    }
}

impl SendLocation<'_> {
    pub fn horizontal_accuracy(mut self, meters: i32) -> Self {
        self.point.accuracy_radius = Some(meters);
        self
    }

    async fn send(self) -> Result<Message> {
        let geo_point = base::InputGeoPoint::InputGeoPoint(Box::new(self.point));
        let media = base::InputMedia::InputMediaGeoPoint(Box::new(types::InputMediaGeoPoint { geo_point }));
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
    use crate::raw::types;

    #[test]
    fn maps_coordinates() {
        let client = Client::offline();
        let send = client.send_location(-100123, 51.5, -0.12).horizontal_accuracy(25).disable_notification();
        let expected = types::InputGeoPoint { lat: 51.5, long: -0.12, accuracy_radius: Some(25) };
        assert_eq!(send.point, expected);
        assert!(send.options.disable_notification);
    }
}
