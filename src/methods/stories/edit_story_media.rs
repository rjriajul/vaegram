use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InputStoryContent, PeerRef, Story};

impl Client {
    pub async fn edit_story_media(
        &self,
        chat_id: impl Into<PeerRef>,
        story_id: i32,
        media: InputStoryContent<'_>,
    ) -> Result<Story> {
        let peer = self.resolve_peer(chat_id).await?;
        let media = media.upload(self).await?;
        let request = functions::stories::EditStory {
            peer,
            id: story_id,
            media: Some(media),
            media_areas: None,
            caption: None,
            entities: None,
            privacy_rules: None,
            music: None,
        };
        Story::from_updates(self, self.invoke(&request).await?)
    }
}
