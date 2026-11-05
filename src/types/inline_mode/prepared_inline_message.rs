/// A message stored by [`Client::save_prepared_inline_message`](crate::Client::save_prepared_inline_message).
///
/// A Mini App can share it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedInlineMessage {
    /// Id of the prepared message.
    pub id: String,
    /// When the prepared message expires, as a Unix timestamp.
    pub expiration_date: i32,
}
