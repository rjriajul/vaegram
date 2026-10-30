use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{PeerRef, TranscribedAudio};

impl Client {
    /// While `pending` is set, the final text arrives later as a raw `UpdateTranscribedAudio`.
    pub async fn transcribe_audio(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> Result<TranscribedAudio> {
        let peer = self.resolve_peer(chat_id).await?;
        let transcribed = self.invoke(&functions::messages::TranscribeAudio { peer, msg_id: message_id }).await?;
        Ok(TranscribedAudio::parse(transcribed))
    }
}
