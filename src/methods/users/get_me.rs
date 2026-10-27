use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};

impl Client {
    pub async fn get_me(&self) -> Result<base::User> {
        let users = self.invoke(&functions::users::GetUsers { id: vec![base::InputUser::InputUserSelf] }).await?;
        users.into_iter().next().ok_or(Error::Unexpected("users.getUsers returned no users"))
    }
}
