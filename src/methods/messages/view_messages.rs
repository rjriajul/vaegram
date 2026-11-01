use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Counts one view of each message, as opening them in an app does; `get_messages_views` only reads the counts.
    pub async fn view_messages(&self, chat_id: impl Into<PeerRef>, message_ids: &[i32]) -> Result<()> {
        if message_ids.is_empty() {
            return Err(Error::InvalidArgument("view_messages needs at least one message id"));
        }
        let request = functions::messages::GetMessagesViews {
            peer: self.resolve_peer(chat_id).await?,
            id: message_ids.to_vec(),
            increment: true,
        };
        self.invoke(&request).await.map(drop)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn views_increment_the_counter() {
        let views = base::messages::MessageViews::MessageViews(Box::new(types::messages::MessageViews {
            views: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        }));
        let (client, mut sent) = Client::scripted(vec![Ok(views.to_bytes())]);
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 1 }));
        client.0.peers.lock().unwrap().insert(7, peer.clone());
        client.view_messages(7, &[3, 4]).await.unwrap();
        let request = functions::messages::GetMessagesViews { peer, id: vec![3, 4], increment: true };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }

    #[tokio::test]
    async fn no_ids_is_refused_without_a_request() {
        let (client, mut sent) = Client::recording();
        assert!(matches!(client.view_messages(7, &[]).await, Err(Error::InvalidArgument(_))));
        assert!(sent.try_recv().is_err());
    }
}
