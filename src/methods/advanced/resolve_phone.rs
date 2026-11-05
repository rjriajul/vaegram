use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::storage::input_peer;
use crate::utils::get_peer_id;

impl Client {
    /// The peer with this phone number: a known one from memory or storage, otherwise asked of Telegram.
    pub async fn resolve_phone(&self, phone_number: &str) -> Result<i64> {
        let phone = phone_number.trim_start_matches('+').replace([' ', '-'], "");
        if let Some(id) = self.0.peers.lock().unwrap().by_phone_number(&phone) {
            return Ok(id);
        }
        match self.0.store.get_peer_by_phone_number(phone.clone()).await {
            Ok(Some(record)) => {
                if input_peer(record.id, record.access_hash).is_some() {
                    let id = record.id;
                    self.0.peers.lock().unwrap().load([record]);
                    return Ok(id);
                }
            }
            Ok(None) => {}
            Err(e) => self.0.events.storage_failed(
                e.kind(),
                format_args!("couldn't look a phone number up in storage, asking Telegram: {e}"),
            ),
        }
        let base::contacts::ResolvedPeer::ResolvedPeer(resolved) =
            self.invoke(&functions::contacts::ResolvePhone { phone }).await?;
        Entities::new(self, resolved.users, resolved.chats);
        Ok(get_peer_id(&resolved.peer))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, types};

    fn user_with_phone(id: i64, phone: &str) -> base::User {
        let mut buf = Vec::new();
        // flags: access_hash (bit 0) and phone (bit 4)
        0b1_0001u32.serialize(&mut buf);
        0u32.serialize(&mut buf);
        id.serialize(&mut buf);
        7i64.serialize(&mut buf);
        phone.to_owned().serialize(&mut buf);
        base::User::User(Box::new(types::User::deserialize_bare(&mut &buf[..]).unwrap()))
    }

    #[tokio::test]
    async fn a_known_number_is_resolved_without_a_request() {
        let (client, mut sent) = Client::scripted(Vec::new());
        crate::dispatcher::Entities::new(&client, vec![user_with_phone(100, "15550001")], Vec::new());
        assert_eq!(client.resolve_phone("+1 555-0001").await.unwrap(), 100);
        assert!(sent.try_recv().is_err(), "a request went out");
    }
}
