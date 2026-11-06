use super::SkipReason;
use crate::errors::Error;
use crate::handlers::HandlerError;

/// Something the client noticed that no request returns: reconnects, dropped packets, updates it couldn't parse
/// or deliver, failed handlers and storage calls. Receive them with [`Client::on_event`](crate::Client::on_event).
#[derive(Debug)]
#[non_exhaustive]
pub enum ClientEvent {
    /// The connection to `dc` came back after `attempt` tries; updates missed meanwhile are being fetched.
    Reconnected {
        /// Id of the data center the main connection is to.
        dc: i32,
        /// How many connection attempts it took, counting the one that worked; 1 means the first try.
        attempt: u32,
    },
    /// A packet from Telegram was thrown away unread, and the connection carries on.
    PacketDropped {
        /// Why: `"failed decryption or the MTProto checks"`, `"bad msg_container"` or `"bad gzip_packed"`.
        reason: &'static str,
    },
    /// An update Telegram pushed couldn't be deserialized and was skipped. An unknown constructor is reported as
    /// [`UnknownConstructor`](Self::UnknownConstructor) instead.
    UpdateParseFailed {
        /// TL constructor id at the start of the update, or 0 when it's shorter than 4 bytes.
        constructor_id: u32,
        /// Length of the update in bytes.
        len: usize,
    },
    /// An update Telegram pushed holds a TL constructor this build's schema doesn't know, usually from a newer
    /// layer, and was skipped.
    UnknownConstructor {
        /// The unknown constructor id; it can sit anywhere inside the update, not only at its start.
        id: u32,
    },
    /// A handler or lifecycle hook returned `Err` and no [`on_error`](crate::Client::on_error) handler is set.
    HandlerFailed {
        /// The error, with the route it came from and the chat of the update.
        error: HandlerError,
    },
    /// Fetching missed updates (`updates.getDifference` or `updates.getChannelDifference`) failed. Updates it gave
    /// up on, if any, follow as [`UpdatesSkipped`](Self::UpdatesSkipped).
    DifferenceFailed {
        /// The error the fetch returned.
        error: Error,
    },
    /// Writing to the file given to `ClientBuilder::record` failed; the message is missing from the recording.
    #[non_exhaustive]
    RecordingFailed {
        /// The I/O error from the recording file.
        error: std::io::Error,
    },
    /// A call to the storage engine failed and `error` says which. The client carries on: a lookup falls back to
    /// asking Telegram, and a save that failed is tried again with the next one. This can fire on the engine's own
    /// thread.
    #[non_exhaustive]
    StorageFailed {
        /// The engine's error; its message names the call that failed.
        error: std::io::Error,
    },
    /// The handlers fell behind and `count` batches of updates didn't fit in the queue. Updates that move pts or
    /// qts are fetched again; `unrecoverable` of the batches also held others (callback queries, typing, ...) that
    /// are gone. More `max_concurrent_handlers` or faster handlers keep it at 0.
    #[non_exhaustive]
    UpdatesDropped {
        /// Batches dropped since the last report.
        count: u32,
        /// How many of those batches held updates a fetch can't bring back. Never more than `count`.
        unrecoverable: u32,
    },
    /// The updates with pts `from..to` of a channel (`chat_id`) or of the account (`None`) will never be
    /// delivered. `from == to` when the fetch stopped before Telegram said how many there were. Gaps in qts
    /// (participant changes, join requests, a bot being stopped, ...) aren't reported.
    #[non_exhaustive]
    UpdatesSkipped {
        /// Bot-API style id of the channel (`-100…`), or `None` for the account's own message box (private chats
        /// and basic groups).
        chat_id: Option<i64>,
        /// Start of the gap: the last pts the client had.
        from: i32,
        /// End of the gap: the pts the next delivered update builds on. Equal to `from` when it isn't known.
        to: i32,
        /// Why the updates were given up on.
        reason: SkipReason,
    },
}
