use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn retract_vote(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> Result<()> {
        let request = functions::messages::SendVote {
            peer: self.resolve_peer(chat_id).await?,
            msg_id: message_id,
            options: Vec::new(),
        };
        self.invoke(&request).await.map(drop)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, functions};

    #[tokio::test]
    async fn retracting_sends_no_options() {
        let (client, mut rx) = Client::recording();
        let _ = client.retract_vote(-5, 9).await;
        let peer = client.resolve_peer(-5).await.unwrap();
        let expected = functions::messages::SendVote { peer, msg_id: 9, options: Vec::new() };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }
}
