use crate::Client;
use crate::errors::Result;
use crate::methods::advanced::ChatPeer;
use crate::raw::{base, functions};
use crate::types::PeerRef;

impl Client {
    pub async fn delete_chat_photo(&self, chat_id: impl Into<PeerRef>) -> Result<()> {
        let chat = self.resolve_chat(&chat_id.into()).await?;
        self.edit_chat_photo(chat, base::InputChatPhoto::InputChatPhotoEmpty).await
    }

    pub(crate) async fn edit_chat_photo(&self, chat: ChatPeer, photo: base::InputChatPhoto) -> Result<()> {
        match chat {
            ChatPeer::Group(chat_id) => {
                self.invoke(&functions::messages::EditChatPhoto { chat_id, photo }).await?;
            }
            ChatPeer::Channel(channel) => {
                self.invoke(&functions::channels::EditPhoto { channel, photo }).await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::client::sent;
    use crate::errors::Error;
    use crate::raw::functions::{InvokeWithBusinessConnection, channels, messages, photos};
    use crate::raw::{Identifiable, base, types};
    use crate::types::Upload;
    use crate::utils::get_channel_id;

    #[tokio::test]
    async fn photo_requests_go_to_the_right_place() {
        let (client, mut rx) = Client::recording();
        let channel = get_channel_id(9);
        {
            let mut peers = client.0.peers.lock().unwrap();
            let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
            peers.insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
            let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
            peers.insert(channel, base::InputPeer::InputPeerChannel(Box::new(peer)));
        }
        let rx = &mut rx;
        assert_eq!(sent(rx, client.delete_chat_photo(-5)).await, messages::EditChatPhoto::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.delete_chat_photo(channel)).await, channels::EditPhoto::CONSTRUCTOR_ID);
        assert!(rx.try_recv().is_err());

        assert_eq!(sent(rx, client.remove_profile_photo()).await, photos::UpdateProfilePhoto::CONSTRUCTOR_ID);
        assert_eq!(
            sent(rx, client.remove_business_account_profile_photo("c", true)).await,
            InvokeWithBusinessConnection::<photos::UpdateProfilePhoto>::CONSTRUCTOR_ID
        );
    }

    #[tokio::test]
    async fn user_chat_ids_are_invalid_for_chat_photos() {
        let (client, mut rx) = Client::recording();
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        let photo = Upload::bytes("p.jpg", b"jpeg");
        assert!(matches!(client.set_chat_photo(7, photo).await, Err(Error::InvalidArgument(_))));
        assert!(matches!(client.delete_chat_photo(7).await, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }
}
