use crate::types::User;

#[derive(Debug, Clone)]
pub enum LoginToken {
    Url { url: String, expires: i32 },
    Authorized(User),
}
