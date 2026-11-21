use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::Chat;

/// A speech-to-text transcription of a voice or video note got new text, as
/// [`Client::transcribe_audio`] started it. To wait for one transcription, use
/// [`Transcription::wait`](crate::types::Transcription::wait).
#[non_exhaustive]
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct TranscriptionUpdated {
    /// The chat the message is in.
    pub chat: Chat,
    /// Id of the transcribed message.
    pub message_id: i32,
    /// Id of the transcription, as [`TranscribedAudio::transcription_id`](crate::types::TranscribedAudio) gives it.
    pub transcription_id: i64,
    /// The text so far. It is partial while `is_pending` is `true`.
    pub text: String,
    /// `true` while Telegram is still transcribing; more updates follow.
    pub is_pending: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
    client: Client,
}

impl TranscriptionUpdated {
    pub(crate) fn parse(client: &Client, raw: &types::UpdateTranscribedAudio, entities: &Entities) -> Self {
        TranscriptionUpdated {
            chat: Chat::parse(&raw.peer, entities, false),
            message_id: raw.msg_id,
            transcription_id: raw.transcription_id,
            text: raw.text.clone(),
            is_pending: raw.pending,
            client: client.clone(),
        }
    }

    /// The [`Client`] that received this update.
    pub fn client(&self) -> &Client {
        &self.client
    }
}
