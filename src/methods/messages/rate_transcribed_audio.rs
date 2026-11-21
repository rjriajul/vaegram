use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Tells Telegram whether the transcription of a voice or video note was good.
    ///
    /// `transcription_id` is [`TranscribedAudio::transcription_id`](crate::types::TranscribedAudio::transcription_id)
    /// of the text [`transcribe_audio`](Client::transcribe_audio) returned. Set `good` to `false` to report a bad
    /// transcript.
    pub async fn rate_transcribed_audio(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        transcription_id: i64,
        good: bool,
    ) -> Result<()> {
        let peer = self.resolve_peer(chat_id).await?;
        let request = functions::messages::RateTranscribedAudio { peer, msg_id: message_id, transcription_id, good };
        accepted(self.invoke(&request).await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn sends_the_rating_of_the_transcription() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes()), Ok(true.to_bytes())]);
        client.rate_transcribed_audio(-5_i64, 9, 77, true).await.unwrap();
        client.rate_transcribed_audio(-5_i64, 9, 77, false).await.unwrap();
        let peer = base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 5 });
        for good in [true, false] {
            let request =
                functions::messages::RateTranscribedAudio { peer: peer.clone(), msg_id: 9, transcription_id: 77, good };
            assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
        }
    }
}
