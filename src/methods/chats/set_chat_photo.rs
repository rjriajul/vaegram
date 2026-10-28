use crate::Client;
use crate::errors::Result;
use crate::methods::users::uploaded;
use crate::raw::{base, types};
use crate::types::{PeerRef, Upload};

#[must_use = "a request does nothing until it is awaited"]
pub struct SetChatPhoto<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    file: Upload<'a>,
    video: bool,
    video_start_ts: Option<f64>,
}

impl Client {
    pub fn set_chat_photo<'a>(&'a self, chat_id: impl Into<PeerRef>, photo: impl Into<Upload<'a>>) -> SetChatPhoto<'a> {
        SetChatPhoto { client: self, chat_id: chat_id.into(), file: photo.into(), video: false, video_start_ts: None }
    }
}

impl SetChatPhoto<'_> {
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

    async fn send(self) -> Result<()> {
        // A wrong-kind chat id fails before anything is uploaded.
        let chat = self.client.resolve_chat(&self.chat_id).await?;
        let (file, video) = uploaded(self.video, self.client.save_file(self.file).await?);
        let video_start_ts = self.video_start_ts;
        let photo = types::InputChatUploadedPhoto { file, video, video_start_ts, video_emoji_markup: None };
        self.client.edit_chat_photo(chat, base::InputChatPhoto::InputChatUploadedPhoto(Box::new(photo))).await
    }
}

into_future!(SetChatPhoto, ());
