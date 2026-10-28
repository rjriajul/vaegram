use super::SkipReason;
use crate::errors::Error;
use crate::handlers::HandlerError;

#[derive(Debug)]
#[non_exhaustive]
pub enum ClientEvent {
    /// The connection to `dc` came back after `attempt` tries; updates missed meanwhile are being fetched.
    Reconnected {
        dc: i32,
        attempt: u32,
    },
    PacketDropped {
        reason: &'static str,
    },
    UpdateParseFailed {
        constructor_id: u32,
        len: usize,
    },
    UnknownConstructor {
        id: u32,
    },
    HandlerFailed {
        error: HandlerError,
    },
    DifferenceFailed {
        error: Error,
    },
    /// Writing to the file given to `ClientBuilder::record` failed; the message is missing from the recording.
    #[non_exhaustive]
    RecordingFailed {
        error: std::io::Error,
    },
    /// The handlers fell behind and `count` batches of updates didn't fit in the queue. Updates that move pts or
    /// qts are fetched again; `unrecoverable` of the batches also held others (callback queries, typing, ...) that
    /// are gone. More `max_concurrent_handlers` or faster handlers keep it at 0.
    #[non_exhaustive]
    UpdatesDropped {
        count: u32,
        unrecoverable: u32,
    },
    /// The updates with pts `from..to` of a channel (`chat_id`) or of the account (`None`) will never be
    /// delivered. `from == to` when the fetch stopped before Telegram said how many there were. Gaps in qts
    /// (participant changes, join requests, a bot being stopped, ...) aren't reported.
    #[non_exhaustive]
    UpdatesSkipped {
        chat_id: Option<i64>,
        from: i32,
        to: i32,
        reason: SkipReason,
    },
}
