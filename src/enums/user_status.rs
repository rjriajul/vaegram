#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserStatus {
    Online,
    Offline,
    Recently,
    LastWeek,
    LastMonth,
    LongAgo,
}
