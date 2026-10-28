use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn delete_business_messages(&self, business_connection_id: &str, message_ids: &[i32]) -> Result<()> {
        let request = functions::messages::DeleteMessages { revoke: true, id: message_ids.to_vec() };
        self.invoke_business(Some(business_connection_id), request).await?;
        Ok(())
    }
}
