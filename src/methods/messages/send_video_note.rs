use super::send_options::{SendOptions, send_builder, send_media};
use crate::Client;
use crate::session::transfer::Report;
use crate::types::messages_and_media::media_setters;
use crate::types::{FileSource, InputMedia, PeerRef};

/// Request built by [`Client::send_video_note`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SendVideoNote<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    media: InputMedia<'a>,
    options: SendOptions,
    progress: Option<Report<'a>>,
}

impl Client {
    /// Sends a round video message.
    ///
    /// Awaiting the request uploads the file if needed and returns the sent [`crate::types::Message`].
    pub fn send_video_note<'a>(
        &'a self,
        chat_id: impl Into<PeerRef>,
        video_note: impl Into<FileSource<'a>>,
    ) -> SendVideoNote<'a> {
        let media = InputMedia::video_note(video_note);
        SendVideoNote { client: self, chat_id: chat_id.into(), media, options: SendOptions::default(), progress: None }
    }
}

media_setters!(SendVideoNote.media: thumb duration length progress);
send_media!(SendVideoNote);
