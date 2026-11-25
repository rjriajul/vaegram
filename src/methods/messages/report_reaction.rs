use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Reports the reaction one member put on a message in a group or channel the account administers.
    ///
    /// `reaction_peer` is the user or chat that reacted.
    pub async fn report_reaction(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        reaction_peer: impl Into<PeerRef>,
    ) -> Result<()> {
        let peer = self.resolve_peer(chat_id).await?;
        let reaction_peer = self.resolve_peer(reaction_peer).await?;
        accepted(self.invoke(&functions::messages::ReportReaction { peer, id: message_id, reaction_peer }).await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn reports_the_reaction_of_a_chat_member() {
        let (client, mut sent) = Client::scripted_raw(vec![Ok(true.to_bytes())]);
        client.report_reaction(-5_i64, 9, -6_i64).await.unwrap();
        let peer = base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 5 });
        let reaction_peer = base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 6 });
        let expected = functions::messages::ReportReaction { peer, id: 9, reaction_peer };
        assert_eq!(sent.try_recv().unwrap(), expected.to_bytes());
    }
}
