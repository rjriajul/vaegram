use crate::raw::base;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscribedAudio {
    pub transcription_id: i64,
    pub text: String,
    pub pending: bool,
    pub trial_remains_num: Option<i32>,
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
