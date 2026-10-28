use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::methods::messages::caption_setters;
use crate::raw::{base, functions};
use crate::types::{InputStoryContent, Story, StoryArea};

#[must_use = "a request does nothing until it is awaited"]
pub struct EditStory<'a> {
    client: &'a Client,
    business_connection_id: String,
    story_id: i32,
    content: InputStoryContent<'a>,
    caption: String,
    parse_mode: Option<ParseMode>,
    areas: Vec<StoryArea>,
}

impl Client {
    /// Replaces a business account's story; caption and areas left unset are cleared.
    pub fn edit_story<'a>(
        &'a self,
        business_connection_id: impl Into<String>,
        story_id: i32,
        content: InputStoryContent<'a>,
    ) -> EditStory<'a> {
        EditStory {
            client: self,
            business_connection_id: business_connection_id.into(),
            story_id,
            content,
            caption: String::new(),
            parse_mode: None,
            areas: Vec::new(),
        }
    }
}

impl EditStory<'_> {
    pub fn caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = caption.into();
        self
    }

    pub fn areas(mut self, areas: impl IntoIterator<Item = StoryArea>) -> Self {
        self.areas = areas.into_iter().collect();
        self
    }

    async fn send(self) -> Result<Story> {
        let client = self.client;
        let peer = client.business_user(&self.business_connection_id).await?;
        let (caption, entities) = client.parse_text(&self.caption, self.parse_mode).await?;
        let request = functions::stories::EditStory {
            peer,
            id: self.story_id,
            media: Some(self.content.upload(client).await?),
            media_areas: Some(self.areas.iter().map(StoryArea::write).collect()),
            caption: Some(caption),
            entities: Some(entities),
            privacy_rules: Some(vec![base::InputPrivacyRule::InputPrivacyValueAllowAll]),
            music: None,
        };
        Story::from_updates(client, client.invoke(&request).await?)
    }
}

caption_setters!(EditStory);
into_future!(EditStory, Story);
