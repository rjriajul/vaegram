use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::BusinessRecipients;

impl Client {
    /// `no_activity_days` is how long a private chat must be quiet to count as inactive: 7, 14, 21 or 28.
    pub async fn set_business_greeting_message(
        &self,
        shortcut_id: i32,
        recipients: &BusinessRecipients,
        no_activity_days: i32,
    ) -> Result<bool> {
        if ![7, 14, 21, 28].contains(&no_activity_days) {
            return Err(Error::InvalidArgument("no_activity_days must be 7, 14, 21 or 28"));
        }
        let recipients = recipients.write(self).await?;
        let message = types::InputBusinessGreetingMessage { shortcut_id, recipients, no_activity_days };
        let message = Some(base::InputBusinessGreetingMessage::InputBusinessGreetingMessage(Box::new(message)));
        self.invoke(&functions::account::UpdateBusinessGreetingMessage { message }).await
    }
}
