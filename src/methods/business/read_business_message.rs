use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn read_business_message(
        &self,
        business_connection_id: &str,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
    ) -> Result<()> {
        let (_, peer) = self.resolve(&chat_id.into()).await?;
        let request = functions::messages::ReadHistory { peer, max_id: message_id };
        self.invoke_business(Some(business_connection_id), request).await?;
        Ok(())
    }
}
