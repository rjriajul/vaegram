use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{Media, PeerRef, UserProfileAudios};

#[must_use = "a request does nothing until it is awaited"]
pub struct GetUserProfileAudios<'a> {
    client: &'a Client,
    user_id: PeerRef,
    offset: i32,
    limit: i32,
}

impl Client {
    pub fn get_user_profile_audios(&self, user_id: impl Into<PeerRef>) -> GetUserProfileAudios<'_> {
        GetUserProfileAudios { client: self, user_id: user_id.into(), offset: 0, limit: 100 }
    }
}

impl GetUserProfileAudios<'_> {
    pub fn offset(mut self, offset: i32) -> Self {
        self.offset = offset;
        self
    }

    pub fn limit(mut self, limit: i32) -> Self {
        self.limit = limit;
        self
    }

    async fn send(self) -> Result<UserProfileAudios> {
        let client = self.client;
        let request = functions::users::GetSavedMusic {
            id: client.resolve_user(&self.user_id).await?,
            offset: self.offset,
            limit: self.limit,
            hash: 0,
        };
        let (total_count, documents) = match client.invoke(&request).await? {
            base::users::SavedMusic::SavedMusic(m) => (m.count, m.documents),
            base::users::SavedMusic::SavedMusicNotModified(m) => (m.count, Vec::new()),
        };
        let audios = documents.iter().filter_map(|d| match Media::from_document(d)? {
            Media::Audio(a) => Some(a),
            _ => None,
        });
        Ok(UserProfileAudios { total_count, audios: audios.collect() })
    }
}

into_future!(GetUserProfileAudios, UserProfileAudios);
