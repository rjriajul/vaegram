use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{FactCheck, PeerRef};

impl Client {
    /// Returns the fact checks of messages of one chat, in the order of `message_ids`.
    ///
    /// A message without a fact check still gets an entry, with no text. An empty `message_ids` sends nothing.
    pub async fn get_fact_check(&self, chat_id: impl Into<PeerRef>, message_ids: &[i32]) -> Result<Vec<FactCheck>> {
        if message_ids.is_empty() {
            return Ok(Vec::new());
        }
        let peer = self.resolve_peer(chat_id).await?;
        let request = functions::messages::GetFactCheck { peer, msg_id: message_ids.to_vec() };
        let checks = self.invoke(&request).await?;
        let entities = Entities::default();
        Ok(checks.iter().map(|check| FactCheck::parse(check, &entities)).collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn reads_one_fact_check_per_message() {
        let checked = types::FactCheck { need_check: false, country: None, text: None, hash: 11 };
        let pending = types::FactCheck { need_check: true, country: None, text: None, hash: 12 };
        let reply = vec![base::FactCheck::FactCheck(Box::new(checked)), base::FactCheck::FactCheck(Box::new(pending))];
        let (client, mut sent) = Client::scripted(vec![Ok(reply.to_bytes())]);
        let checks = client.get_fact_check(-5_i64, &[3, 4]).await.unwrap();
        assert_eq!(checks.iter().map(|c| (c.needs_check, c.hash)).collect::<Vec<_>>(), [(false, 11), (true, 12)]);
        let peer = base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 5 });
        let expected = functions::messages::GetFactCheck { peer, msg_id: vec![3, 4] };
        assert_eq!(sent.try_recv().unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn no_ids_send_nothing() {
        let (client, mut sent) = Client::recording();
        assert!(client.get_fact_check(-5_i64, &[]).await.unwrap().is_empty());
        assert!(sent.try_recv().is_err());
    }
}
