use super::send_options::{SendOptions, caption_setters, send_builder};
use crate::{
    Client,
    enums::ParseMode,
    errors::Result,
    raw::{base, types},
    types::{Message, PeerRef, Text},
};

/// Request built by [`Client::send_shared_widget`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SendSharedWidget<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    widget_id: i64,
    chat_data: Option<String>,
    caption: Text,
    parse_mode: Option<ParseMode>,
    options: SendOptions,
}
impl Client {
    /// Sends again a shared widget this bot created, by its [`Widget::id`](crate::types::Widget::id). Only bots can
    /// send widgets. Awaiting the request returns the sent [`Message`].
    pub fn send_shared_widget(&self, chat_id: impl Into<PeerRef>, widget_id: i64) -> SendSharedWidget<'_> {
        SendSharedWidget {
            client: self,
            chat_id: chat_id.into(),
            widget_id,
            chat_data: None,
            caption: Text::new(),
            parse_mode: None,
            options: SendOptions::default(),
        }
    }
}
impl SendSharedWidget<'_> {
    /// Sets the chat-specific data.
    pub fn chat_data(mut self, data: impl Into<String>) -> Self {
        self.chat_data = Some(data.into());
        self
    }
    /// Sets the caption.
    pub fn caption(mut self, caption: impl Into<Text>) -> Self {
        self.caption = caption.into();
        self
    }
    /// Shows the caption above the widget.
    pub fn show_caption_above_media(mut self) -> Self {
        self.options.invert_media = true;
        self
    }
    async fn send(self) -> Result<Message> {
        let media = types::InputMediaWidgetId { widget_id: self.widget_id, chat_data: self.chat_data };
        self.client
            .send_media(
                self.chat_id,
                base::InputMedia::InputMediaWidgetId(Box::new(media)),
                self.caption,
                self.parse_mode,
                self.options,
            )
            .await
    }
}
caption_setters!(SendSharedWidget);
send_builder!(SendSharedWidget);
