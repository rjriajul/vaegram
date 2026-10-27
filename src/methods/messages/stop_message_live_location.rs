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
}

impl Client {
    pub fn stop_message_live_location(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
    ) -> StopMessageLiveLocation<'_> {
        StopMessageLiveLocation { client: self, chat_id: chat_id.into(), message_id, reply_markup: None }
    }
}

impl StopMessageLiveLocation<'_> {
    pub fn reply_markup(mut self, markup: impl Into<ReplyMarkup>) -> Self {
        self.reply_markup = Some(markup.into());
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
        edit_live_location(self.client, &self.chat_id, self.message_id, media, self.reply_markup).await
    }
}

into_future!(StopMessageLiveLocation, Message);
