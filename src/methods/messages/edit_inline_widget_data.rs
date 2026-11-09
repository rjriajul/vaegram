use crate::{Client, errors::Result, methods::accepted, raw::functions, types::InlineMessageId};

/// Request built by [`Client::edit_inline_widget_data`].
#[must_use = "a request does nothing until it is awaited"]
pub struct EditInlineWidgetData<'a> {
    client: &'a Client,
    inline_message_id: InlineMessageId,
    public_data: Option<String>,
    chat_data: Option<String>,
}
impl Client {
    /// Edits data on an inline widget.
    pub fn edit_inline_widget_data(&self, inline_message_id: InlineMessageId) -> EditInlineWidgetData<'_> {
        EditInlineWidgetData { client: self, inline_message_id, public_data: None, chat_data: None }
    }
}
impl EditInlineWidgetData<'_> {
    /// Sets public data; an empty string clears it. Omit to leave it unchanged.
    pub fn public_data(mut self, data: impl Into<String>) -> Self {
        self.public_data = Some(data.into());
        self
    }
    /// Sets chat data; an empty string clears it. Omit to leave it unchanged.
    pub fn chat_data(mut self, data: impl Into<String>) -> Self {
        self.chat_data = Some(data.into());
        self
    }
    async fn send(self) -> Result<()> {
        let dc_id = self.inline_message_id.dc_id();
        let request = functions::messages::EditInlineBotMessageWidgetData {
            id: self.inline_message_id.0,
            public_data: self.public_data,
            chat_data: self.chat_data,
        };
        accepted(self.client.invoke_inline_on_behalf(dc_id, None, request).await?)
    }
}
into_future!(EditInlineWidgetData, ());
