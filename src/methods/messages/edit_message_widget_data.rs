use crate::{Client, errors::Result, raw::functions, types::PeerRef};

/// Request built by [`Client::edit_message_widget_data`].
#[must_use = "a request does nothing until it is awaited"]
pub struct EditMessageWidgetData<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    public_data: Option<String>,
    chat_data: Option<String>,
}
impl Client {
    /// Edits data on a widget in a chat.
    pub fn edit_message_widget_data(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> EditMessageWidgetData<'_> {
        EditMessageWidgetData { client: self, chat_id: chat_id.into(), message_id, public_data: None, chat_data: None }
    }
}
impl EditMessageWidgetData<'_> {
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
        let peer = self.client.resolve(&self.chat_id).await?.1;
        self.client
            .invoke(&functions::messages::EditMessageWidgetData {
                peer,
                id: self.message_id,
                public_data: self.public_data,
                chat_data: self.chat_data,
            })
            .await?;
        Ok(())
    }
}
into_future!(EditMessageWidgetData, ());
