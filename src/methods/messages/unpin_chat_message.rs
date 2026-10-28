use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
pub struct UnpinChatMessage<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    business_connection_id: Option<String>,
}

impl Client {
    pub fn unpin_chat_message(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> UnpinChatMessage<'_> {
        UnpinChatMessage { client: self, chat_id: chat_id.into(), message_id, business_connection_id: None }
    }
}

impl UnpinChatMessage<'_> {
    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.business_connection_id = Some(id.into());
        self
    }

    async fn send(self) -> Result<()> {
        let request = functions::messages::UpdatePinnedMessage {
            silent: false,
            unpin: true,
            pm_oneside: false,
            peer: self.client.resolve_peer(self.chat_id).await?,
            id: self.message_id,
        };
        self.client.invoke_business(self.business_connection_id.as_deref(), request).await?;
        Ok(())
    }
}

into_future!(UnpinChatMessage, ());

impl crate::methods::OnBehalf for UnpinChatMessage<'_> {
    fn business_connection(&mut self) -> &mut Option<String> {
        &mut self.business_connection_id
    }
}
