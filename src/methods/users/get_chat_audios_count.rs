use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    /// How many audios a user has saved to their profile.
    pub async fn get_chat_audios_count(&self, chat_id: impl Into<PeerRef>) -> Result<i32> {
        Ok(self.get_user_profile_audios(chat_id).limit(1).await?.total_count)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::functions::users;
    use crate::raw::{Serializable, base, types};

    #[tokio::test]
    async fn counts_from_one_saved_music_slice() {
        let reply = base::users::SavedMusic::SavedMusicNotModified(types::users::SavedMusicNotModified { count: 4 });
        let (client, mut sent) = Client::scripted(vec![Ok(reply.to_bytes())]);
        let user = types::InputPeerUser { user_id: 7, access_hash: 3 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        assert_eq!(client.get_chat_audios_count(7).await.unwrap(), 4);
        let id = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 7, access_hash: 3 }));
        let expected = users::GetSavedMusic { id, offset: 0, limit: 1, hash: 0 };
        assert_eq!(sent.try_recv().unwrap(), expected.to_bytes());
    }
}
