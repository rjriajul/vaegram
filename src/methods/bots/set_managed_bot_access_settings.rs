use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// `added_user_ids` are ignored when `is_access_restricted` is false.
    pub async fn set_managed_bot_access_settings(
        &self,
        bot_id: i64,
        is_access_restricted: bool,
        added_user_ids: &[i64],
    ) -> Result<bool> {
        let bot = self.resolve_user(&bot_id.into()).await?;
        let add_users = if is_access_restricted {
            let mut users = Vec::with_capacity(added_user_ids.len());
            for &id in added_user_ids {
                users.push(self.resolve_user(&id.into()).await?);
            }
            Some(users)
        } else {
            None
        };
        let request = functions::bots::EditAccessSettings { restricted: is_access_restricted, bot, add_users };
        self.invoke(&request).await
    }
}
