use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;

impl Client {
    /// Deletes messages in a chat of a business connection, for both sides.
    ///
    /// Takes 1 to 100 ids, otherwise it fails with `Error::InvalidArgument`.
    pub async fn delete_business_messages(
        &self,
        business_connection_id: impl Into<String>,
        message_ids: &[i32],
    ) -> Result<()> {
        let business_connection_id: String = business_connection_id.into();
        if !(1..=100).contains(&message_ids.len()) {
            return Err(Error::InvalidArgument("deletes take 1 to 100 message ids"));
        }
        let request = functions::messages::DeleteMessages { revoke: true, id: message_ids.to_vec() };
        self.invoke_business(Some(&business_connection_id), request).await?;
        Ok(())
    }
}
