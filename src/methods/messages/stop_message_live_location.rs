use crate::Client;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::{Message, PeerRef, ReplyMarkup};

use super::edit_message_live_location::edit_live_location;

#[must_use = "a request does nothing until it is awaited"]
pub struct StopMessageLiveLocation<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    reply_markup: Option<ReplyMarkup>,
    business_connection_id: Option<String>,
}

impl Client {
    pub fn stop_message_live_location(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
    ) -> StopMessageLiveLocation<'_> {
        StopMessageLiveLocation {
            client: self,
            chat_id: chat_id.into(),
            message_id,
            reply_markup: None,
            business_connection_id: None,
        }
    }
}

impl StopMessageLiveLocation<'_> {
    pub fn reply_markup(mut self, markup: impl Into<ReplyMarkup>) -> Self {
        self.reply_markup = Some(markup.into());
        self
    }

    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.business_connection_id = Some(id.into());
        self
    }

    async fn send(self) -> Result<Message> {
        let media = types::InputMediaGeoLive {
            stopped: true,
            geo_point: base::InputGeoPoint::InputGeoPointEmpty,
            heading: None,
            period: None,
            proximity_notification_radius: None,
        };
        let business = self.business_connection_id.as_deref();
        edit_live_location(self.client, &self.chat_id, self.message_id, media, self.reply_markup, business).await
    }
}

into_future!(StopMessageLiveLocation, Message);

impl crate::methods::OnBehalf for StopMessageLiveLocation<'_> {
    fn business_connection(&mut self) -> &mut Option<String> {
        &mut self.business_connection_id
    }
}
