use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::User;

impl Client {
    /// Returns the logged-in account.
    ///
    /// ```no_run
    /// # async fn f(client: &vaegram::Client) -> vaegram::Result<()> {
    /// let me = client.get_me().await?;
    /// println!("{} ({})", me.first_name, me.id);
    /// # Ok(()) }
    /// ```
    pub async fn get_me(&self) -> Result<User> {
        Ok(User::from_raw(self, self.get_me_raw().await?))
    }

    /// The account as Telegram sent it, for the fields the friendly `User` leaves out (the access hash).
    pub(crate) async fn get_me_raw(&self) -> Result<base::User> {
        let users = self.invoke(&functions::users::GetUsers { id: vec![base::InputUser::InputUserSelf] }).await?;
        users.into_iter().next().ok_or(Error::Unexpected("users.getUsers returned no users"))
    }
}
