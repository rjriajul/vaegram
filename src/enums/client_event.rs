use std::sync::Arc;

use super::SkipReason;
use crate::errors::Error;
use crate::handlers::{HandlerError, UpdateKind};

/// Something the client noticed that no request returns: reconnects, dropped packets, updates it couldn't parse
/// or deliver, failed handlers and storage calls. Receive them with [`Client::on_event`](crate::Client::on_event).
///
/// Errors are behind an [`Arc`], so an event clones cheaply, for example to send it to several channels.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ClientEvent {
    /// A connection to `dc` came back after `attempt` tries. When it's the main connection, updates missed
    /// meanwhile are being fetched.
    #[non_exhaustive]
    Reconnected {
        /// Id of the data center whose connection came back; file-transfer connections report theirs too.
        dc: i32,
        /// How many connection attempts it took, counting the one that worked; 1 means the first try.
        attempt: u32,
    },
    /// A file-transfer connection to `dc` couldn't be opened. The transfer goes on over the connections that did;
    /// only when none did does it fail, with one of the errors.
    #[non_exhaustive]
    ConnectFailed {
        /// Id of the data center the connection was for.
        dc: i32,
        /// Why it couldn't be opened.
        error: Arc<Error>,
    },
    /// A packet from Telegram, or one message of a container, was thrown away unread, and the connection carries on.
    #[non_exhaustive]
    PacketDropped {
        /// Why: `"failed decryption or the MTProto checks"`, `"msg_id seen before, even or outside the time window"`
        /// (the replay check; Telegram can also send a message again that was already taken in),
        /// `"bad msg_container"`, `"msg_container holds a newer msg_id"` or `"bad gzip_packed"`.
        reason: &'static str,
    },
    /// An update Telegram pushed couldn't be deserialized and was skipped. An unknown constructor is reported as
    /// [`UnknownConstructor`](Self::UnknownConstructor) instead.
    #[non_exhaustive]
    UpdateParseFailed {
        /// TL constructor id at the start of the update, or 0 when it's shorter than 4 bytes.
        constructor_id: u32,
        /// Length of the update in bytes.
        len: usize,
    },
    /// An update Telegram pushed holds a TL constructor this build's schema doesn't know, usually from a newer
    /// layer, and was skipped.
    #[non_exhaustive]
    UnknownConstructor {
        /// The unknown constructor id; it can sit anywhere inside the update, not only at its start.
        id: u32,
    },
    /// A handler or lifecycle hook returned `Err` and no [`on_error`](crate::Client::on_error) handler is set.
    #[non_exhaustive]
    HandlerFailed {
        /// The error, with the route it came from and the chat, message, user and query of the update.
        error: Arc<HandlerError>,
    },
    /// A handler, its filter, a raw handler or a lifecycle hook panicked. The panic is caught where it happened, so
    /// update delivery carries on: a panicking handler stops the later groups for its update as an `Err` does, a
    /// panicking filter counts as not matching, an [`ordered`] chat moves on to its next update and the handler's
    /// [`max_concurrent_handlers`] slot is given back. [`on_error`](crate::Client::on_error) doesn't receive
    /// panics, and the process's panic hook has already printed the panic by the time this is sent.
    ///
    /// [`ordered`]: crate::ClientBuilder::ordered
    /// [`max_concurrent_handlers`]: crate::ClientBuilder::max_concurrent_handlers
    #[non_exhaustive]
    HandlerPanicked {
        /// The route of the handler that panicked, as in [`HandlerError::kind`].
        kind: UpdateKind,
        /// Bot-API style id of the update's chat, as in [`HandlerError::chat_id`].
        chat_id: Option<i64>,
        /// The panic's message when it is a `&str` or a `String`, as `panic!`, `unwrap` and `expect` give.
        message: Option<String>,
    },
    /// Fetching missed updates (`updates.getDifference` or `updates.getChannelDifference`) failed. Updates it gave
    /// up on, if any, follow as [`UpdatesSkipped`](Self::UpdatesSkipped).
    #[non_exhaustive]
    DifferenceFailed {
        /// The error the fetch returned.
        error: Arc<Error>,
    },
    /// Writing to the file given to `ClientBuilder::record` failed; the message is missing from the recording.
    #[non_exhaustive]
    RecordingFailed {
        /// The I/O error from the recording file.
        error: Arc<std::io::Error>,
    },
    /// A call to the storage engine failed and `error` says which. The client carries on: a lookup falls back to
    /// asking Telegram, and a save that failed is tried again with the next one. This can fire on the engine's own
    /// thread.
    #[non_exhaustive]
    StorageFailed {
        /// The engine's error; its message names the call that failed.
        error: Arc<std::io::Error>,
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
    /// delivered. `from == to` when the fetch stopped before Telegram said how many there were, or when there was
    /// nothing to fetch from ([`SkipReason::NoPts`]: no fetch was made). A report for the account also stands for
    /// its qts updates (participant changes, join requests, a bot being stopped, ...), whose own numbers it doesn't
    /// give: `from..to` are pts. Of those, the ones held behind a missing one went out past it, and the ones dropped
    /// because too many were held are still ahead of the client's qts, so a later fetch of the account brings them.
    #[non_exhaustive]
    UpdatesSkipped {
        /// Bot-API style id of the channel (`-100…`), or `None` for the account's own message box (private chats
        /// and basic groups).
        chat_id: Option<i64>,
        /// Start of the gap: the last pts the client had, or 0 when it had none ([`SkipReason::NoPts`]).
        from: i32,
        /// End of the gap: the pts the next delivered update builds on. Equal to `from` when it isn't known.
        to: i32,
        /// Why the updates were given up on.
        reason: SkipReason,
    },
}
