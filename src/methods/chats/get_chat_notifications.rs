use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{ChatNotifications, PeerRef};

impl Client {
    pub async fn get_chat_notifications(&self, chat_id: impl Into<PeerRef>) -> Result<ChatNotifications> {
        let peer = base::InputNotifyPeer::InputNotifyPeer(Box::new(types::InputNotifyPeer {
            peer: self.resolve_peer(chat_id).await?,
        }));
        Ok(ChatNotifications::parse(self.invoke(&functions::account::GetNotifySettings { peer }).await?))
    }
}
