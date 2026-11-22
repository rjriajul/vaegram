use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::TranscriptionUpdated;

impl Client {
    /// Registers a handler for new text in speech-to-text transcriptions of voice and video notes that passes
    /// `filter`. Each step of a pending transcription comes as one update; the last has `is_pending` unset.
    ///
    /// To wait for the transcription one [`transcribe_audio`](Client::transcribe_audio) call started, use
    /// [`Transcription::wait`](crate::types::Transcription::wait) instead.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_transcription<F, Fut, T>(&self, filter: Filter<TranscriptionUpdated>, f: F) -> HandlerRef<'_>
    where
        F: Fn(Client, TranscriptionUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::Transcription, |r, id| r.transcription.push(id, filter, callback(f)))
    }
}
