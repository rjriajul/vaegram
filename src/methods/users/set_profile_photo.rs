use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{PeerRef, Upload};

/// Request built by [`Client::set_profile_photo`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SetProfilePhoto<'a> {
    client: &'a Client,
    business_connection_id: Option<String>,
    file: Upload<'a>,
    video: bool,
    video_start_ts: Option<f64>,
    public: bool,
    bot: Option<PeerRef>,
}

impl Client {
    /// Uploads a file and makes it your profile photo.
    ///
    /// Awaiting it uploads the file, then sets it. It returns once Telegram has accepted the new photo.
    pub fn set_profile_photo<'a>(&'a self, photo: impl Into<Upload<'a>>) -> SetProfilePhoto<'a> {
        SetProfilePhoto::new(self, None, photo.into())
    }
}

impl<'a> SetProfilePhoto<'a> {
    pub(crate) fn new(client: &'a Client, business_connection_id: Option<String>, file: Upload<'a>) -> Self {
        SetProfilePhoto {
            client,
            business_connection_id,
            file,
            video: false,
            video_start_ts: None,
            public: false,
            bot: None,
        }
    }

    pub(crate) fn bot(mut self, bot: PeerRef) -> Self {
        self.bot = Some(bot);
        self
    }

    /// Uploads the file as a profile video instead of a photo.
    pub fn video(mut self) -> Self {
        self.video = true;
        self
    }

    /// Second of the video to use as the static preview. Implies [`video`](SetProfilePhoto::video).
    pub fn video_start_ts(mut self, seconds: f64) -> Self {
        self.video = true;
        self.video_start_ts = Some(seconds);
        self
    }

    /// Sets the public photo shown to users the account hides its main photo from.
    pub fn public(mut self) -> Self {
        self.public = true;
        self
    }

    async fn send(self) -> Result<()> {
        // A business account's photo isn't sent through the connection, which may be on another data center than
        // the uploaded parts: the bot uploads it naming the account, as it does for a bot it owns.
        let owner = match &self.business_connection_id {
            Some(connection) => Some(PeerRef::from(self.client.business_home(connection).await?.user_id)),
            None => self.bot,
        };
        let bot = match &owner {
            Some(owner) => Some(self.client.resolve_user(owner).await?),
            None => None,
        };
        let (file, video) = uploaded(self.video, self.client.save_file(self.file).await?);
        let request = upload_request(bot, self.public, self.video_start_ts, file, video);
        self.client.invoke(&request).await?;
        Ok(())
    }
}

into_future!(SetProfilePhoto, ());

pub(crate) fn uploaded(video: bool, file: base::InputFile) -> (Option<base::InputFile>, Option<base::InputFile>) {
    if video { (None, Some(file)) } else { (Some(file), None) }
}

fn upload_request(
    bot: Option<base::InputUser>,
    fallback: bool,
    video_start_ts: Option<f64>,
    file: Option<base::InputFile>,
    video: Option<base::InputFile>,
) -> functions::photos::UploadProfilePhoto {
    functions::photos::UploadProfilePhoto { fallback, bot, file, video, video_start_ts, video_emoji_markup: None }
}

#[cfg(test)]
mod tests {
    use super::upload_request;
    use crate::Client;
    use crate::raw::functions::photos::UploadProfilePhoto;
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::types::Upload;

    #[test]
    fn bot_photo_sends_the_bot() {
        let bot = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 9, access_hash: 1 }));
        let request = upload_request(Some(bot.clone()), false, None, None, None);
        assert_eq!(request.bot, Some(bot));
        assert_eq!(upload_request(None, false, None, None, None).bot, None);
    }

    #[tokio::test]
    async fn a_business_photo_is_uploaded_naming_the_account() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes())]);
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        client.0.caches.know_business("c", 7, 4);
        let _ = client.set_business_account_profile_photo("c", Upload::bytes("p.jpg", b"jpeg")).await;
        let _part = sent.try_recv().expect("no part sent");
        // Not wrapped: the parts are on this client's data center, the connection's is 4.
        let body = sent.try_recv().expect("no photo sent");
        assert_eq!(body[..4], UploadProfilePhoto::CONSTRUCTOR_ID.to_le_bytes());
        let account = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 7, access_hash: 1 })).to_bytes();
        assert!(body.windows(account.len()).any(|w| w == account));
    }
}
