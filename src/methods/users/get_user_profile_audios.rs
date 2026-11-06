use crate::Client;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::{Audio, Media, PeerRef};

/// The audios a user saved to their profile, built by [`Client::get_user_profile_audios`].
///
/// Awaiting it collects the audios into a `Vec`, up to [`limit`](GetUserProfileAudios::limit);
/// [`next`](GetUserProfileAudios::next) reads them one at a time. Audios are fetched 100 to a page.
#[must_use = "a pager does nothing until it is awaited or read with next()"]
pub struct GetUserProfileAudios<'a> {
    client: &'a Client,
    user_id: PeerRef,
    user: Option<base::InputUser>,
    total: Option<i32>,
    pages: OffsetPages<Audio, i32>,
}

impl Client {
    /// Lists the audios a user saved to their profile.
    pub fn get_user_profile_audios(&self, user_id: impl Into<PeerRef>) -> GetUserProfileAudios<'_> {
        let pages = OffsetPages::starting_at(100, 0);
        GetUserProfileAudios { client: self, user_id: user_id.into(), user: None, total: None, pages }
    }
}

impl GetUserProfileAudios<'_> {
    /// Skips this many audios from the start of the list.
    pub fn offset(mut self, offset: i32) -> Self {
        self.pages.set_offset(offset);
        self
    }

    /// Stops after this many audios in total. The default reads them all.
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    /// Returns how many audios the profile holds, ignoring `limit` and `offset`.
    ///
    /// Before the first page this reads one, which `next` then returns from without another request.
    pub async fn total(&mut self) -> Result<i32> {
        if self.total.is_none() {
            self.fetch(self.pages.limit().unwrap_or(1)).await?;
        }
        Ok(self.total.unwrap_or(0))
    }

    /// Returns the next audio, or `None` once the audios or the limit run out.
    pub async fn next(&mut self) -> Result<Option<Audio>> {
        loop {
            if let Some(audio) = self.pages.pop() {
                return Ok(Some(audio));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            self.fetch(limit).await?;
        }
    }

    async fn fetch(&mut self, limit: i32) -> Result<()> {
        let id = match &self.user {
            Some(user) => user.clone(),
            None => self.client.resolve_user(&self.user_id).await?,
        };
        self.user = Some(id.clone());
        let offset = self.pages.offset().copied().unwrap_or(0);
        let request = functions::users::GetSavedMusic { id, offset, limit, hash: 0 };
        let (total, documents) = match self.client.invoke(&request).await? {
            base::users::SavedMusic::SavedMusic(m) => (m.count, m.documents),
            base::users::SavedMusic::SavedMusicNotModified(m) => (m.count, Vec::new()),
        };
        self.total = Some(total);
        let audios = documents.iter().filter_map(|d| match Media::from_document(d)? {
            Media::Audio(a) => Some(a),
            _ => None,
        });
        self.pages.push_counted(audios.collect(), documents.len(), total);
        Ok(())
    }
}

pager!(GetUserProfileAudios, Audio);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::functions::users::GetSavedMusic;
    use crate::raw::{Serializable, base, types};

    fn not_modified(count: i32) -> crate::Result<Vec<u8>> {
        Ok(base::users::SavedMusic::SavedMusicNotModified(types::users::SavedMusicNotModified { count }).to_bytes())
    }

    #[tokio::test]
    async fn an_empty_page_ends_the_list_and_keeps_the_offset() {
        let (client, mut sent) = Client::scripted(vec![not_modified(4)]);
        let user = types::InputPeerUser { user_id: 7, access_hash: 3 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        let mut audios = client.get_user_profile_audios(7).offset(2).limit(5);
        assert!(audios.next().await.unwrap().is_none());
        assert_eq!(audios.total().await.unwrap(), 4);
        let id = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 7, access_hash: 3 }));
        assert_eq!(sent.try_recv().unwrap(), GetSavedMusic { id, offset: 2, limit: 5, hash: 0 }.to_bytes());
        assert!(sent.try_recv().is_err());
    }
}
