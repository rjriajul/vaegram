use crate::Client;
use crate::enums::StoriesPrivacyRules;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{PeerRef, Story};

/// Request built by [`Client::edit_story_privacy`]. Awaiting it returns the edited [`Story`].
#[must_use = "a request does nothing until it is awaited"]
pub struct EditStoryPrivacy<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    story_id: i32,
    privacy: StoriesPrivacyRules,
}

impl Client {
    /// Changes who can see a story and returns the edited [`Story`].
    pub fn edit_story_privacy(
        &self,
        chat_id: impl Into<PeerRef>,
        story_id: i32,
        privacy: StoriesPrivacyRules,
    ) -> EditStoryPrivacy<'_> {
        EditStoryPrivacy { client: self, chat_id: chat_id.into(), story_id, privacy }
    }
}

impl EditStoryPrivacy<'_> {
    async fn send(self) -> Result<Story> {
        let client = self.client;
        let request = functions::stories::EditStory {
            peer: client.resolve_peer(self.chat_id).await?,
            id: self.story_id,
            media: None,
            media_areas: None,
            caption: None,
            entities: None,
            privacy_rules: Some(client.input_privacy_rules(&self.privacy.rules()).await?),
            music: None,
        };
        Story::from_updates(client, client.invoke(&request).await?)
    }
}

into_future!(EditStoryPrivacy, Story);
