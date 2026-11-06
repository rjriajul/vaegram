use crate::Client;
use crate::errors::Result;
use crate::methods::pages::wire_limit;
use crate::raw::{base, functions};
use crate::types::{PeerRef, Photo, UserProfilePhotos};

/// Request built by [`Client::get_chat_photos`].
#[must_use = "a request does nothing until it is awaited"]
pub struct GetChatPhotos<'a> {
    client: &'a Client,
    user_id: PeerRef,
    offset: i32,
    limit: i32,
}

impl Client {
    /// Fetches a user's profile photos, newest first.
    ///
    /// Awaiting it returns a [`UserProfilePhotos`] with the total count and up to
    /// [`limit`](GetChatPhotos::limit) photos, 100 by default.
    pub fn get_chat_photos(&self, user_id: impl Into<PeerRef>) -> GetChatPhotos<'_> {
        GetChatPhotos { client: self, user_id: user_id.into(), offset: 0, limit: 100 }
    }
}

impl GetChatPhotos<'_> {
    /// Skips this many photos from the newest.
    pub fn offset(mut self, offset: i32) -> Self {
        self.offset = offset;
        self
    }

    /// Caps how many photos one call returns. Defaults to 100.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = wire_limit(limit);
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
