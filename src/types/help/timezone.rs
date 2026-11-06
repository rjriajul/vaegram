/// A timezone Telegram accepts for business hours.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Timezone {
    /// Timezone identifier, for example `Europe/Amsterdam`.
    pub id: String,
    /// Display name of the timezone.
    pub name: String,
    /// Offset from UTC in seconds.
    pub utc_offset_seconds: i32,
}
