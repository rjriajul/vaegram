use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::messages_and_media::transcription::Waiter;
use crate::types::{PeerRef, TranscribedAudio, Transcription};

impl Client {
    /// Transcribes the audio of a voice or video note message to text.
    ///
    /// While `is_pending` is set, the final text arrives later: [`Transcription::wait`] waits for it, and
    /// [`on_transcription`](Client::on_transcription) handlers get every step.
    pub async fn transcribe_audio(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> Result<Transcription> {
        let peer = self.resolve_peer(chat_id).await?;
        // Before the request: the final text may arrive before its reply does.
        let waiter = Waiter::new(self, message_id);
        let transcribed = self.invoke(&functions::messages::TranscribeAudio { peer, msg_id: message_id }).await?;
        Ok(Transcription::new(TranscribedAudio::parse(transcribed), waiter))
    }
}

#[cfg(test)]
mod tests {
    use std::task::Poll;
    use std::time::Duration;

    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};

    fn reply(pending: bool, text: &str) -> Vec<u8> {
        let audio = types::messages::TranscribedAudio {
            pending,
            transcription_id: 77,
            text: text.into(),
            trial_remains_num: None,
            trial_remains_until_date: None,
        };
        base::messages::TranscribedAudio::TranscribedAudio(Box::new(audio)).to_bytes()
    }

    fn update(transcription_id: i64, pending: bool, text: &str) -> base::Update {
        base::Update::UpdateTranscribedAudio(Box::new(types::UpdateTranscribedAudio {
            pending,
            peer: base::Peer::PeerChat(types::PeerChat { chat_id: 5 }),
            msg_id: 9,
            transcription_id,
            text: text.into(),
        }))
    }

    #[tokio::test]
    async fn wait_gets_a_final_text_that_beat_the_reply() {
        let (client, mut sent) = Client::scripted(vec![Ok(reply(true, "hel"))]);
        let call = client.transcribe_audio(-5_i64, 9);
        tokio::pin!(call);
        // Poll the call by hand until its request is out. The scripted reply is then waiting, but the call hasn't
        // read it, so the result fed below arrives before the call sees its reply.
        let mut body = None;
        for _ in 0..100 {
            let ready = std::future::poll_fn(|cx| Poll::Ready(call.as_mut().poll(cx).is_ready())).await;
            assert!(!ready, "the call finished before it was answered");
            tokio::task::yield_now().await;
            body = sent.try_recv().ok();
            if body.is_some() {
                break;
            }
        }
        let peer = base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 5 });
        assert_eq!(body, Some(functions::messages::TranscribeAudio { peer, msg_id: 9 }.to_bytes()));
        let transcriptions = &client.0.handlers.transcriptions;
        transcriptions.feed(&update(78, false, "another transcription"));
        transcriptions.feed(&update(77, true, "hello wo"));
        transcriptions.feed(&update(77, false, "hello world"));
        let mut transcription = call.await.unwrap();
        assert!(transcription.is_pending);
        assert_eq!(transcription.text, "hel");
        assert_eq!(transcription.wait(Duration::from_secs(5)).await.unwrap(), "hello world");
        assert!(!transcription.is_pending);
        assert_eq!(transcription.text, "hello world");
        drop(transcription);
        // The dropped transcription stopped waiting.
        assert!(transcriptions.is_empty());
    }

    #[tokio::test]
    async fn wait_times_out_and_a_finished_one_returns_at_once() {
        let (client, _sent) = Client::scripted(vec![Ok(reply(true, "hel")), Ok(reply(false, "done"))]);
        let mut pending = client.transcribe_audio(-5_i64, 9).await.unwrap();
        let waited = pending.wait(Duration::from_millis(10)).await;
        assert!(matches!(waited, Err(Error::ListenerTimeout)));
        let mut finished = client.transcribe_audio(-5_i64, 9).await.unwrap();
        assert_eq!(finished.wait(Duration::ZERO).await.unwrap(), "done");
    }
}
