use std::ops::Deref;
use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::mpsc;

use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::base;
use crate::types::TranscribedAudio;

/// What an `updateTranscribedAudio` says about one transcription.
#[derive(Debug)]
struct Transcribed {
    transcription_id: i64,
    text: String,
    is_pending: bool,
}

#[derive(Default)]
struct Waiting {
    next_id: u64,
    // The waiter's id, the message it transcribes, and where its updates go.
    waiters: Vec<(u64, i32, mpsc::UnboundedSender<Transcribed>)>,
}

/// The `transcribe_audio` calls whose transcription may still change. Each gets every transcription update for its
/// message id from before its request is sent, so a result that beats the reply isn't lost; the transcription id
/// sorts out the rest.
#[derive(Default)]
pub(crate) struct Transcriptions(Mutex<Waiting>);

impl Transcriptions {
    fn register(&self, message_id: i32) -> (u64, mpsc::UnboundedReceiver<Transcribed>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut waiting = self.0.lock().unwrap();
        waiting.next_id += 1;
        let id = waiting.next_id;
        waiting.waiters.push((id, message_id, tx));
        (id, rx)
    }

    fn remove(&self, id: u64) {
        self.0.lock().unwrap().waiters.retain(|(i, _, _)| *i != id);
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.0.lock().unwrap().waiters.is_empty()
    }

    /// Hands a transcription update to the waiters for its message.
    pub(crate) fn feed(&self, update: &base::Update) {
        let base::Update::UpdateTranscribedAudio(u) = update else { return };
        let waiting = self.0.lock().unwrap();
        for (_, message_id, tx) in &waiting.waiters {
            if *message_id == u.msg_id {
                let (text, is_pending) = (u.text.clone(), u.pending);
                let _ = tx.send(Transcribed { transcription_id: u.transcription_id, text, is_pending });
            }
        }
    }
}

/// Registered with the client from just before the request until the [`Transcription`] is dropped.
#[derive(Debug)]
pub(crate) struct Waiter {
    id: u64,
    client: Client,
    rx: mpsc::UnboundedReceiver<Transcribed>,
}

impl Waiter {
    pub(crate) fn new(client: &Client, message_id: i32) -> Waiter {
        let (id, rx) = client.0.handlers.transcriptions.register(message_id);
        Waiter { id, client: client.clone(), rx }
    }
}

impl Drop for Waiter {
    fn drop(&mut self) {
        self.client.0.handlers.transcriptions.remove(self.id);
    }
}

/// A transcription [`Client::transcribe_audio`] started. It reads as the [`TranscribedAudio`] Telegram answered
/// with; while that is pending, [`wait`](Self::wait) waits for the final text.
#[non_exhaustive]
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Transcription {
    audio: TranscribedAudio,
    #[cfg_attr(feature = "serde", serde(skip))]
    waiter: Waiter,
}

impl Transcription {
    pub(crate) fn new(audio: TranscribedAudio, waiter: Waiter) -> Transcription {
        Transcription { audio, waiter }
    }

    /// Waits up to `timeout` for the final text and returns it; at once if the transcription isn't pending. After
    /// it returns, this reads as the finished transcription.
    ///
    /// The text arrives as an update, so a [`run`](Client::run) (or another dispatcher) has to be taking updates.
    /// Fails with [`Error::ListenerTimeout`] when the time runs out first; the wait can be tried again.
    pub async fn wait(&mut self, timeout: Duration) -> Result<String> {
        if !self.audio.is_pending {
            return Ok(self.audio.text.clone());
        }
        let (id, rx) = (self.audio.transcription_id, &mut self.waiter.rx);
        let finished = tokio::time::timeout(timeout, async {
            while let Some(t) = rx.recv().await {
                if t.transcription_id == id && !t.is_pending {
                    return Some(t.text);
                }
            }
            None
        });
        match finished.await {
            Ok(Some(text)) => {
                (self.audio.text, self.audio.is_pending) = (text, false);
                Ok(self.audio.text.clone())
            }
            Ok(None) => Err(Error::Disconnected),
            Err(_) => Err(Error::ListenerTimeout),
        }
    }
}

impl Deref for Transcription {
    type Target = TranscribedAudio;

    fn deref(&self) -> &TranscribedAudio {
        &self.audio
    }
}
