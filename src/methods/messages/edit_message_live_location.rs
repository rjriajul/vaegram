use super::edit_message_text::edit_request;
use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{Message, PeerRef, ReplyMarkup};

#[must_use = "a request does nothing until it is awaited"]
pub struct EditMessageLiveLocation<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    latitude: f64,
    longitude: f64,
    horizontal_accuracy: Option<i32>,
    heading: Option<i32>,
    proximity_alert_radius: Option<i32>,
    live_period: Option<i32>,
    reply_markup: Option<ReplyMarkup>,
    business_connection_id: Option<String>,
}

impl Client {
    pub fn edit_message_live_location(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        latitude: f64,
        longitude: f64,
    ) -> EditMessageLiveLocation<'_> {
        EditMessageLiveLocation {
            client: self,
            chat_id: chat_id.into(),
            message_id,
            latitude,
            longitude,
            horizontal_accuracy: None,
            heading: None,
            proximity_alert_radius: None,
            live_period: None,
            reply_markup: None,
            business_connection_id: None,
        }
    }
}

impl EditMessageLiveLocation<'_> {
    pub fn horizontal_accuracy(mut self, meters: i32) -> Self {
        self.horizontal_accuracy = Some(meters);
        self
    }

    pub fn heading(mut self, degrees: i32) -> Self {
        self.heading = Some(degrees);
        self
    }

    pub fn proximity_alert_radius(mut self, meters: i32) -> Self {
        self.proximity_alert_radius = Some(meters);
        self
    }

    pub fn live_period(mut self, seconds: i32) -> Self {
        self.live_period = Some(seconds);
        self
    }

    pub fn reply_markup(mut self, markup: impl Into<ReplyMarkup>) -> Self {
        self.reply_markup = Some(markup.into());
        self
    }

    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.business_connection_id = Some(id.into());
        self
    }

    async fn send(self) -> Result<Message> {
        let point = types::InputGeoPoint {
            lat: self.latitude,
            long: self.longitude,
            accuracy_radius: self.horizontal_accuracy,
        };
        let media = types::InputMediaGeoLive {
            stopped: false,
            geo_point: base::InputGeoPoint::InputGeoPoint(Box::new(point)),
            heading: self.heading,
            period: self.live_period,
            proximity_notification_radius: self.proximity_alert_radius,
        };
        let business = self.business_connection_id.as_deref();
        edit_live_location(self.client, &self.chat_id, self.message_id, media, self.reply_markup, business).await
    }
}

into_future!(EditMessageLiveLocation, Message);

impl crate::methods::OnBehalf for EditMessageLiveLocation<'_> {
    fn business_connection(&mut self) -> &mut Option<String> {
        &mut self.business_connection_id
    }
}

pub(super) async fn edit_live_location(
    client: &Client,
    chat_id: &PeerRef,
    message_id: i32,
    media: types::InputMediaGeoLive,
    reply_markup: Option<ReplyMarkup>,
    business_connection_id: Option<&str>,
) -> Result<Message> {
    let (chat_id, peer) = client.resolve(chat_id).await?;
    let request = functions::messages::EditMessage {
        media: Some(base::InputMedia::InputMediaGeoLive(Box::new(media))),
        reply_markup: reply_markup.as_ref().map(ReplyMarkup::write),
        ..edit_request(peer, message_id)
    };
    let (updates, _) = client.invoke_business(business_connection_id, request).await?;
    Message::from_updates(client, updates, chat_id, (String::new(), None))
}
