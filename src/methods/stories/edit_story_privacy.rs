use crate::Client;
use crate::enums::StoriesPrivacyRules;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{PeerRef, Story};

impl Client {
    pub async fn edit_story_privacy(
        &self,
        chat_id: impl Into<PeerRef>,
        story_id: i32,
        privacy: StoriesPrivacyRules,
    ) -> Result<Story> {
        let request = functions::stories::EditStory {
            peer: self.resolve_peer(chat_id).await?,
            id: story_id,
            media: None,
            media_areas: None,
            caption: None,
            entities: None,
            privacy_rules: Some(self.input_privacy_rules(&privacy.rules()).await?),
            music: None,
        };
        Story::from_updates(self, self.invoke(&request).await?)
    }
}
