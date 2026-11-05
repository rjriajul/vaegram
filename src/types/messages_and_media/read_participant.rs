/// Someone who has read a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadParticipant {
    /// Id of the user.
    pub user_id: i64,
    /// When they read it, as a Unix timestamp.
    pub date: i32,
}
