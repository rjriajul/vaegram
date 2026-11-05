use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};

impl Client {
    /// Returns the logged-in account as a raw `User`.
    ///
    /// ```no_run
    /// # async fn f(client: &vaegram::Client) -> vaegram::Result<()> {
    /// let me = client.get_me().await?;
    /// # Ok(()) }
    /// ```
    pub async fn get_me(&self) -> Result<base::User> {
        let users = self.invoke(&functions::users::GetUsers { id: vec![base::InputUser::InputUserSelf] }).await?;
        users.into_iter().next().ok_or(Error::Unexpected("users.getUsers returned no users"))
    }
}
