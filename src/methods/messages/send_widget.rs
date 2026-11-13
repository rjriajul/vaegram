use super::send_options::{SendOptions, caption_setters, send_builder};
use crate::{
    Client,
    enums::ParseMode,
    errors::Result,
    raw::{base, types},
    types::{Message, PeerRef, Text},
};

/// Request built by [`Client::send_widget`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SendWidget<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    html: String,
    width: i32,
    height: i32,
    public_data: Option<String>,
    chat_data: Option<String>,
    allow_queries: bool,
    shared_widget: bool,
    caption: Text,
    parse_mode: Option<ParseMode>,
    options: SendOptions,
}
impl Client {
    /// Sends a message with an HTML widget: one self-contained page, up to 32 KB and 4 KB after zlib compression.
    ///
    /// Only bots can send widgets; a user account gets `INPUT_MEDIA_WIDGET_NOT_ALLOWED`. Telegram also decides where
    /// they may go: in production today only private chats, and sent widgets are protected content. A refusal comes
    /// back as `Error::Rpc`, e.g. `INPUT_MEDIA_WIDGET_NOT_ALLOWED` or `INPUT_MEDIA_WIDGET_TOO_LARGE`. Awaiting the
    /// request returns the sent [`Message`].
    pub fn send_widget(&self, chat_id: impl Into<PeerRef>, html: impl Into<String>) -> SendWidget<'_> {
        SendWidget {
            client: self,
            chat_id: chat_id.into(),
            html: html.into(),
            width: 0,
            height: 0,
            public_data: None,
            chat_data: None,
            allow_queries: false,
            shared_widget: false,
            caption: Text::new(),
            parse_mode: None,
            options: SendOptions::default(),
        }
    }
}
impl SendWidget<'_> {
    /// Width in logical pixels. Without it the widget fills the message bubble's width.
    pub fn width(mut self, width: i32) -> Self {
        self.width = width;
        self
    }
    /// Height in logical pixels. Without it the height isn't fixed.
    pub fn height(mut self, height: i32) -> Self {
        self.height = height;
        self
    }
    /// Data shown wherever the widget is, up to 1024 bytes. A shared widget has one for all its messages.
    pub fn public_data(mut self, data: impl Into<String>) -> Self {
        self.public_data = Some(data.into());
        self
    }
    /// Data for this message only, up to 1024 bytes. Forwards don't carry it.
    pub fn chat_data(mut self, data: impl Into<String>) -> Self {
        self.chat_data = Some(data.into());
        self
    }
    /// Lets the widget send queries to the bot, which arrive at [`Client::on_widget_query`].
    pub fn allow_queries(mut self) -> Self {
        self.allow_queries = true;
        self
    }
    /// Makes the widget shared: the sent [`Widget`](crate::types::Widget) has an `id` to send it again with
    /// [`Client::send_shared_widget`], and with [`allow_queries`](Self::allow_queries) its forwards can send
    /// queries too.
    pub fn is_shared(mut self) -> Self {
        self.shared_widget = true;
        self
    }
    /// Caption shown with the widget.
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
        let media = types::InputMediaWidget {
            allow_queries: self.allow_queries,
            shared_widget: self.shared_widget,
            width: self.width,
            height: self.height,
            html: self.html,
            public_data: self.public_data,
            chat_data: self.chat_data,
        };
        self.client
            .send_media(
                self.chat_id,
                base::InputMedia::InputMediaWidget(Box::new(media)),
                self.caption,
                self.parse_mode,
                self.options,
            )
            .await
    }
}
caption_setters!(SendWidget);
send_builder!(SendWidget);

#[cfg(test)]
mod tests {
    use crate::{
        Client,
        raw::{Serializable, base, types},
    };
    #[tokio::test]
    async fn sends_widget_flags_and_automatic_width() {
        let (client, mut sent) = Client::scripted(Vec::new());
        let _ = client.send_widget(-5, "<p>x</p>").height(25).allow_queries().is_shared().await;
        let body = sent.try_recv().unwrap();
        let media = base::InputMedia::InputMediaWidget(Box::new(types::InputMediaWidget {
            allow_queries: true,
            shared_widget: true,
            width: 0,
            height: 25,
            html: "<p>x</p>".into(),
            public_data: None,
            chat_data: None,
        }))
        .to_bytes();
        assert!(body.windows(media.len()).any(|w| w == media));
    }
}
