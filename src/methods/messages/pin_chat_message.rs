use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
pub struct PinChatMessage<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    disable_notification: bool,
    both_sides: bool,
    business_connection_id: Option<String>,
}

impl Client {
    /// In private chats the pin is only for you unless [`PinChatMessage::both_sides`] is set.
    pub fn pin_chat_message(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> PinChatMessage<'_> {
        PinChatMessage {
            client: self,
            chat_id: chat_id.into(),
            message_id,
            disable_notification: false,
            both_sides: false,
            business_connection_id: None,
        }
    }
}

impl PinChatMessage<'_> {
    pub fn disable_notification(mut self) -> Self {
        self.disable_notification = true;
        self
    }

    pub fn both_sides(mut self) -> Self {
        self.both_sides = true;
        self
    }

    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.business_connection_id = Some(id.into());
        self
    }

    async fn send(self) -> Result<()> {
        let request = functions::messages::UpdatePinnedMessage {
            silent: self.disable_notification,
            unpin: false,
            pm_oneside: !self.both_sides,
            peer: self.client.resolve_peer(self.chat_id).await?,
            id: self.message_id,
        };
        self.client.invoke_business(self.business_connection_id.as_deref(), request).await?;
        Ok(())
    }
}

into_future!(PinChatMessage, ());

impl crate::methods::OnBehalf for PinChatMessage<'_> {
    fn business_connection(&mut self) -> &mut Option<String> {
        &mut self.business_connection_id
    }
}
