use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{PeerRef, Photo, UserProfilePhotos};

#[must_use = "a request does nothing until it is awaited"]
pub struct GetChatPhotos<'a> {
    client: &'a Client,
    user_id: PeerRef,
    offset: i32,
    limit: i32,
}

impl Client {
    /// A user's profile photos, newest first: up to 100 per call, skipping `offset` of them.
    pub fn get_chat_photos(&self, user_id: impl Into<PeerRef>) -> GetChatPhotos<'_> {
        GetChatPhotos { client: self, user_id: user_id.into(), offset: 0, limit: 100 }
    }
}

impl GetChatPhotos<'_> {
    pub fn offset(mut self, offset: i32) -> Self {
        self.offset = offset;
        self
    }

    pub fn limit(mut self, limit: i32) -> Self {
        self.limit = limit;
        self
    }

    async fn send(self) -> Result<UserProfilePhotos> {
        let user_id = self.client.resolve_user(&self.user_id).await?;
        let request = functions::photos::GetUserPhotos { user_id, offset: self.offset, max_id: 0, limit: self.limit };
        let (total_count, photos) = match self.client.invoke(&request).await? {
            // A full list means nothing follows, so the total is everything up to its end.
            base::photos::Photos::Photos(p) => (self.offset + p.photos.len() as i32, p.photos),
            base::photos::Photos::PhotosSlice(p) => (p.count, p.photos),
        };
        Ok(UserProfilePhotos { total_count, photos: photos.iter().filter_map(Photo::parse).collect() })
    }
}

into_future!(GetChatPhotos, UserProfilePhotos);
