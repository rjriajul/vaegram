/// Why [`ClientEvent::UpdatesSkipped`](super::ClientEvent::UpdatesSkipped) happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SkipReason {
    /// Too many to fetch: Telegram jumped to its current state, for a channel along with its latest messages.
    TooLong,
    /// Telegram refused to send them, e.g. after the bot left the channel; the error came just before as
    /// `DifferenceFailed`.
    Refused,
    /// Fetching them kept failing.
    GaveUp,
}
