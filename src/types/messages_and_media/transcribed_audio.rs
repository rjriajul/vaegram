use crate::raw::base;

/// A speech-to-text transcription of a voice or video note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscribedAudio {
    /// Id of the transcription.
    pub transcription_id: i64,
    /// The transcribed text. It can still be partial while `pending` is `true`.
    pub text: String,
    /// `true` while Telegram is still transcribing.
    pub pending: bool,
    /// Free transcriptions left in the trial. `None` if the account isn't on a trial.
    pub trial_remains_num: Option<i32>,
    /// Unix time when the trial allowance resets. `None` if the account isn't on a trial.
    pub trial_remains_until_date: Option<i32>,
}

impl TranscribedAudio {
    pub(crate) fn parse(raw: base::messages::TranscribedAudio) -> TranscribedAudio {
        let base::messages::TranscribedAudio::TranscribedAudio(t) = raw;
        TranscribedAudio {
            transcription_id: t.transcription_id,
            text: t.text,
            pending: t.pending,
            trial_remains_num: t.trial_remains_num,
            trial_remains_until_date: t.trial_remains_until_date,
        }
    }
}
