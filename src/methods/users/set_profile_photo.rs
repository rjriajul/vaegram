use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{PeerRef, Upload};

/// Request built by [`Client::set_profile_photo`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SetProfilePhoto<'a> {
    client: &'a Client,
    business_connection_id: Option<&'a str>,
    file: Upload<'a>,
    video: bool,
    video_start_ts: Option<f64>,
    public: bool,
    bot: Option<PeerRef>,
}

impl Client {
    /// Uploads a new profile photo for your account.
    ///
    /// Awaiting it returns once Telegram has accepted the upload.
    pub fn set_profile_photo<'a>(&'a self, photo: impl Into<Upload<'a>>) -> SetProfilePhoto<'a> {
        SetProfilePhoto::new(self, None, photo.into())
    }
}

impl<'a> SetProfilePhoto<'a> {
    pub(crate) fn new(client: &'a Client, business_connection_id: Option<&'a str>, file: Upload<'a>) -> Self {
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
        let bot = match &self.bot {
            Some(bot) => Some(self.client.resolve_user(bot).await?),
            None => None,
        };
        let (file, video) = uploaded(self.video, self.client.save_file(self.file).await?);
        let request = upload_request(bot, self.public, self.video_start_ts, file, video);
        self.client.invoke_business(self.business_connection_id, request).await?;
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
    use crate::raw::{base, types};

    #[test]
    fn bot_photo_sends_the_bot() {
        let bot = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 9, access_hash: 1 }));
        let request = upload_request(Some(bot.clone()), false, None, None, None);
        assert_eq!(request.bot, Some(bot));
        assert_eq!(upload_request(None, false, None, None, None).bot, None);
    }
}
