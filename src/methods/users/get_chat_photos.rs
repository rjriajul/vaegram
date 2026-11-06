use crate::Client;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::{PeerRef, Photo};

/// A user's profile photos, newest first, built by [`Client::get_chat_photos`].
///
/// Awaiting it collects the photos into a `Vec`, up to [`limit`](GetChatPhotos::limit);
/// [`next`](GetChatPhotos::next) reads them one at a time. Photos are fetched 100 to a page.
#[must_use = "a pager does nothing until it is awaited or read with next()"]
pub struct GetChatPhotos<'a> {
    client: &'a Client,
    user_id: PeerRef,
    user: Option<base::InputUser>,
    total: Option<i32>,
    pages: OffsetPages<Photo, i32>,
}

impl Client {
    /// Lists a user's profile photos, newest first.
    pub fn get_chat_photos(&self, user_id: impl Into<PeerRef>) -> GetChatPhotos<'_> {
        let pages = OffsetPages::starting_at(100, 0);
        GetChatPhotos { client: self, user_id: user_id.into(), user: None, total: None, pages }
    }
}

impl GetChatPhotos<'_> {
    /// Skips this many photos from the newest.
    pub fn offset(mut self, offset: i32) -> Self {
        self.pages.set_offset(offset);
        self
    }

    /// Stops after this many photos in total. The default reads them all.
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    /// Returns how many profile photos the user has, ignoring `limit` and `offset`.
    ///
    /// Before the first page this reads one, which `next` then returns from without another request.
    pub async fn total(&mut self) -> Result<i32> {
        if self.total.is_none() {
            self.fetch(self.pages.limit().unwrap_or(1)).await?;
        }
        Ok(self.total.unwrap_or(0))
    }

    /// Returns the next photo, or `None` once the photos or the limit run out.
    pub async fn next(&mut self) -> Result<Option<Photo>> {
        loop {
            if let Some(photo) = self.pages.pop() {
                return Ok(Some(photo));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            self.fetch(limit).await?;
        }
    }

    async fn fetch(&mut self, limit: i32) -> Result<()> {
        let user_id = match &self.user {
            Some(user) => user.clone(),
            None => self.client.resolve_user(&self.user_id).await?,
        };
        self.user = Some(user_id.clone());
        let offset = self.pages.offset().copied().unwrap_or(0);
        let request = functions::photos::GetUserPhotos { user_id, offset, max_id: 0, limit };
        let (total, photos) = match self.client.invoke(&request).await? {
            // A full list means nothing follows, so the total is everything up to its end.
            base::photos::Photos::Photos(p) => (offset.saturating_add(p.photos.len() as i32), p.photos),
            base::photos::Photos::PhotosSlice(p) => (p.count, p.photos),
        };
        self.total = Some(total);
        let read = photos.len();
        self.pages.push_counted(photos.iter().filter_map(Photo::parse).collect(), read, total);
        Ok(())
    }
}

pager!(GetChatPhotos, Photo);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::functions::photos::GetUserPhotos;
    use crate::raw::{Serializable, base, types};

    fn photo(id: i64) -> base::Photo {
        let size = types::PhotoSize { r#type: "x".into(), w: 8, h: 8, size: 64 };
        let photo = types::Photo {
            has_stickers: false,
            id,
            access_hash: 2,
            file_reference: vec![3],
            date: 0,
            sizes: vec![base::PhotoSize::PhotoSize(Box::new(size))],
            video_sizes: None,
            dc_id: 2,
        };
        base::Photo::Photo(Box::new(photo))
    }

    fn slice(count: i32, ids: &[i64]) -> crate::Result<Vec<u8>> {
        let photos = ids.iter().map(|&id| photo(id)).collect();
        let slice = types::photos::PhotosSlice { count, photos, users: Vec::new() };
        Ok(base::photos::Photos::PhotosSlice(Box::new(slice)).to_bytes())
    }

    fn user_7(replies: Vec<crate::Result<Vec<u8>>>) -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let (client, sent) = Client::scripted(replies);
        let user = types::InputPeerUser { user_id: 7, access_hash: 3 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        (client, sent)
    }

    fn request(offset: i32, limit: i32) -> Vec<u8> {
        let user_id = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 7, access_hash: 3 }));
        GetUserPhotos { user_id, offset, max_id: 0, limit }.to_bytes()
    }

    #[tokio::test]
    async fn pages_skip_what_was_read() {
        let (client, mut sent) = user_7(vec![slice(5, &[1, 2]), slice(5, &[3, 4, 5])]);
        let photos = client.get_chat_photos(7).offset(1).await.unwrap();
        assert_eq!(photos.iter().map(|p| p.id).collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
        assert_eq!(sent.try_recv().unwrap(), request(1, 100));
        assert_eq!(sent.try_recv().unwrap(), request(3, 100));
        // 1 skipped plus 5 read covers the 5 Telegram counted and more, so nothing else is asked.
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn the_limit_stops_reading() {
        let (client, mut sent) = user_7(vec![slice(500, &[1, 2]), slice(500, &[3])]);
        let mut photos = client.get_chat_photos(7).limit(3);
        let mut ids = Vec::new();
        while let Some(photo) = photos.next().await.unwrap() {
            ids.push(photo.id);
        }
        assert_eq!((ids, photos.total().await.unwrap()), (vec![1, 2, 3], 500));
        assert_eq!(sent.try_recv().unwrap(), request(0, 3));
        assert_eq!(sent.try_recv().unwrap(), request(2, 1));
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn an_empty_page_ends_the_list() {
        let (client, mut sent) = user_7(vec![slice(9, &[])]);
        assert!(client.get_chat_photos(7).await.unwrap().is_empty());
        assert_eq!(sent.try_recv().unwrap(), request(0, 100));
        assert!(sent.try_recv().is_err());
    }
}
