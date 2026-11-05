use crate::types::Photo;

/// Users the user picked in answer to a [`crate::types::KeyboardButtonRequestUsers`] button.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UsersShared {
    /// The id set on the request button.
    pub request_id: i32,
    /// The users that were picked.
    pub users: Vec<SharedUser>,
}

/// Fields other than `user_id` are only set when the button asked for them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SharedUser {
    /// Id of the user.
    pub user_id: i64,
    /// First name. `None` unless the button asked for it.
    pub first_name: Option<String>,
    /// Last name. `None` unless the button asked for it, or the user has none.
    pub last_name: Option<String>,
    /// Username. `None` unless the button asked for it, or the user has none.
    pub username: Option<String>,
    /// Profile photo. `None` unless the button asked for it, or the user has none.
    pub photo: Option<Photo>,
}
