/// A user's last-seen status as Telegram reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum UserStatus {
    /// Online now.
    Online,
    /// Offline, with an exact last-seen time.
    Offline,
    /// Last seen recently. The exact time is hidden.
    Recently,
    /// Last seen within a week. The exact time is hidden.
    LastWeek,
    /// Last seen within a month. The exact time is hidden.
    LastMonth,
    /// No status set, shown as last seen a long time ago.
    LongAgo,
}
