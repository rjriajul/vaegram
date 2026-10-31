use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    pub async fn get_chat_gifts_count(&self, chat_id: impl Into<PeerRef>) -> Result<i32> {
        Ok(self.get_chat_gifts(chat_id).limit(1).await?.total_count)
    }
}

#[cfg(test)]
mod tests {
    use super::super::send_payment_form::tests::client;
    use crate::raw::{Serializable, base, functions, types};

    fn page(count: i32) -> Vec<u8> {
        let page = types::payments::SavedStarGifts {
            count,
            chat_notifications_enabled: None,
            gifts: Vec::new(),
            next_offset: None,
            chats: Vec::new(),
            users: Vec::new(),
        };
        base::payments::SavedStarGifts::SavedStarGifts(Box::new(page)).to_bytes()
    }

    fn request(peer: base::InputPeer) -> Vec<u8> {
        let request = functions::payments::GetSavedStarGifts {
            exclude_unsaved: false,
            exclude_saved: false,
            exclude_unlimited: false,
            exclude_unique: false,
            sort_by_value: false,
            exclude_upgradable: false,
            exclude_unupgradable: false,
            peer_color_available: false,
            exclude_hosted: false,
            peer,
            collection_id: None,
            offset: String::new(),
            limit: 1,
        };
        request.to_bytes()
    }

    #[tokio::test]
    async fn counts_the_gifts_of_a_chat_with_one_small_page() {
        let (client, mut sent) = client(vec![page(42), page(3)]);
        assert_eq!(client.get_chat_gifts_count(5).await.unwrap(), 42);
        let user = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 5, access_hash: 6 }));
        assert_eq!(sent.try_recv().unwrap(), request(user));
        assert_eq!(client.get_chat_gifts_count("me").await.unwrap(), 3);
        assert_eq!(sent.try_recv().unwrap(), request(base::InputPeer::InputPeerSelf));
    }
}
