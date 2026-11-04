use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PeerRef;

impl Client {
    /// Removes the profile photo of a bot the account owns.
    pub async fn remove_bot_profile_photo(&self, bot_id: impl Into<PeerRef>) -> Result<()> {
        let bot = self.resolve_user(&bot_id.into()).await?;
        let id = base::InputPhoto::InputPhotoEmpty;
        self.invoke(&functions::photos::UpdateProfilePhoto { fallback: false, bot: Some(bot), id }).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn remove_bot_photo_sends_empty_photo() {
        let (client, mut rx) = Client::recording();
        let bot = types::InputPeerUser { user_id: 9, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(9, base::InputPeer::InputPeerUser(Box::new(bot)));
        let _ = client.remove_bot_profile_photo(9_i64).await;
        let bot = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 9, access_hash: 1 }));
        let expected = functions::photos::UpdateProfilePhoto {
            fallback: false,
            bot: Some(bot),
            id: base::InputPhoto::InputPhotoEmpty,
        };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }
}
