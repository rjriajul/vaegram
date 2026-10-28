use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Upload;

#[must_use = "a request does nothing until it is awaited"]
pub struct SetProfilePhoto<'a> {
    client: &'a Client,
    business_connection_id: Option<&'a str>,
    file: Upload<'a>,
    video: bool,
    video_start_ts: Option<f64>,
    public: bool,
}

impl Client {
    pub fn set_profile_photo<'a>(&'a self, photo: impl Into<Upload<'a>>) -> SetProfilePhoto<'a> {
        SetProfilePhoto::new(self, None, photo.into())
    }
}

impl<'a> SetProfilePhoto<'a> {
    pub(crate) fn new(client: &'a Client, business_connection_id: Option<&'a str>, file: Upload<'a>) -> Self {
        SetProfilePhoto { client, business_connection_id, file, video: false, video_start_ts: None, public: false }
    }

    pub fn video(mut self) -> Self {
        self.video = true;
        self
    }

    /// Second of the video to use as the static preview; implies `.video()`.
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
        let (file, video) = uploaded(self.video, self.client.save_file(self.file).await?);
        let request = functions::photos::UploadProfilePhoto {
            fallback: self.public,
            bot: None,
            file,
            video,
            video_start_ts: self.video_start_ts,
            video_emoji_markup: None,
        };
        self.client.invoke_business(self.business_connection_id, request).await?;
        Ok(())
    }
}

into_future!(SetProfilePhoto, ());

pub(crate) fn uploaded(video: bool, file: base::InputFile) -> (Option<base::InputFile>, Option<base::InputFile>) {
    if video { (None, Some(file)) } else { (Some(file), None) }
}
