use crate::types::Photo;

/// A chat the user picked in response to a
/// [`KeyboardButtonRequestChat`](crate::types::KeyboardButtonRequestChat) button.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChatShared {
    /// The `request_id` of the button that was pressed.
    pub request_id: i32,
    /// Id of the chosen chat.
    pub chat_id: i64,
    /// The chat's title. `None` unless the button asked for it.
    pub title: Option<String>,
    /// The chat's username. `None` if the button didn't ask for it or the chat has none.
    pub username: Option<String>,
    /// The chat's photo. `None` if the button didn't ask for it or the chat has none.
    pub photo: Option<Photo>,
}
