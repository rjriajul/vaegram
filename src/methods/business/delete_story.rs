use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn delete_story(&self, business_connection_id: &str, story_id: i32) -> Result<()> {
        let peer = self.business_user(business_connection_id).await?;
        let request = functions::stories::DeleteStories { peer, id: vec![story_id] };
        self.invoke_business(Some(business_connection_id), request).await?;
        Ok(())
    }
}
