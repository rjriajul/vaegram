use crate::types::Photo;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct UsersShared {
    pub request_id: i32,
    pub users: Vec<SharedUser>,
}

/// Fields other than `user_id` are only set when the button asked for them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SharedUser {
    pub user_id: i64,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub photo: Option<Photo>,
}
