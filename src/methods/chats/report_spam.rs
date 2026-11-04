use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Reports a chat that wrote first as spam, as the "Report spam" bar of a new chat does.
    pub async fn report_spam(&self, chat_id: impl Into<PeerRef>) -> Result<bool> {
        let request = functions::messages::ReportSpam { peer: self.resolve_peer(chat_id).await? };
        self.invoke(&request).await
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::functions::messages::ReportSpam;
    use crate::raw::{Serializable, base, types};

    #[tokio::test]
    async fn report_spam_names_the_peer() {
        let refused = Err(Error::Rpc(RpcError::new(400, "PEER_ID_INVALID")));
        let (client, mut rx) = Client::scripted(vec![Ok(true.to_bytes()), refused]);
        let user = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 2 }));
        client.0.peers.lock().unwrap().insert(7, user.clone());
        assert!(client.report_spam(7).await.unwrap());
        assert_eq!(rx.try_recv().unwrap(), ReportSpam { peer: user }.to_bytes());
        let result = client.report_spam(7).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "PEER_ID_INVALID"), "{result:?}");
    }
}
