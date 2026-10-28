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
    /// The handlers fell behind and `count` batches of updates didn't fit in the queue; they're being refetched.
    UpdatesDropped {
        count: u32,
    },
}
