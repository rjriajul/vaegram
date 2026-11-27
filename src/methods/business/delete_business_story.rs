use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Deletes a story posted by the business account behind a connection.
    pub async fn delete_business_story(&self, business_connection_id: impl Into<String>, story_id: i32) -> Result<()> {
        let business_connection_id: String = business_connection_id.into();
        // The Bot API names the story by the connection alone.
        #[cfg(feature = "bot-api")]
        if let Some(api) = self.0.bot_api.as_deref() {
            return api.delete_story(&business_connection_id, story_id).await;
        }
        let peer = self.business_user(&business_connection_id).await?;
        let request = functions::stories::DeleteStories { peer, id: vec![story_id] };
        self.invoke_business(Some(&business_connection_id), request).await?;
        Ok(())
    }
}
