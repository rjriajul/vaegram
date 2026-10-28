use crate::Client;
use crate::enums::ChatAction;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
pub struct SendChatAction<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    action: ChatAction,
    business_connection_id: Option<String>,
}

impl Client {
    pub fn send_chat_action(&self, chat_id: impl Into<PeerRef>, action: ChatAction) -> SendChatAction<'_> {
        SendChatAction { client: self, chat_id: chat_id.into(), action, business_connection_id: None }
    }
}

impl SendChatAction<'_> {
    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.business_connection_id = Some(id.into());
        self
    }

    async fn send(self) -> Result<()> {
        let peer = self.client.resolve_peer(self.chat_id).await?;
        let request = functions::messages::SetTyping { peer, top_msg_id: None, action: self.action.write() };
        self.client.invoke_business(self.business_connection_id.as_deref(), request).await?;
        Ok(())
    }
}

into_future!(SendChatAction, ());

impl crate::methods::OnBehalf for SendChatAction<'_> {
    fn business_connection(&mut self) -> &mut Option<String> {
        &mut self.business_connection_id
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::ChatAction;
    use crate::raw::{base, types};

    #[tokio::test]
    async fn a_business_chat_action_goes_through_the_connection() {
        let (client, mut rx) = Client::recording();
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        assert!(client.send_chat_action(7, ChatAction::Typing).business_connection_id("c").await.is_err());
        assert_eq!(rx.try_recv().expect("no request sent")[..4], 0xdd289f8e_u32.to_le_bytes());
    }
}
