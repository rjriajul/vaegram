use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::methods::messages::caption_setters;
use crate::raw::{base, functions};
use crate::types::{InputStoryContent, Story, StoryArea, Text};

/// Request built by [`Client::edit_business_story`]. Awaiting it returns the edited [`Story`].
#[must_use = "a request does nothing until it is awaited"]
pub struct EditBusinessStory<'a> {
    client: &'a Client,
    business_connection_id: String,
    story_id: i32,
    content: InputStoryContent<'a>,
    caption: Text,
    parse_mode: Option<ParseMode>,
    areas: Vec<StoryArea>,
}

impl Client {
    /// Replaces the content of a story posted by the business account behind a connection.
    ///
    /// A caption or areas left unset are cleared, and the story becomes visible to everyone.
    pub fn edit_business_story<'a>(
        &'a self,
        business_connection_id: impl Into<String>,
        story_id: i32,
        content: InputStoryContent<'a>,
    ) -> EditBusinessStory<'a> {
        EditBusinessStory {
            client: self,
            business_connection_id: business_connection_id.into(),
            story_id,
            content,
            caption: Text::new(),
            parse_mode: None,
            areas: Vec::new(),
        }
    }
}

impl EditBusinessStory<'_> {
    /// Adds a caption under the story, parsed with the request's parse mode.
    pub fn caption(mut self, caption: impl Into<Text>) -> Self {
        self.caption = caption.into();
        self
    }

    /// Places interactive areas on the story.
    pub fn areas(mut self, areas: impl IntoIterator<Item = StoryArea>) -> Self {
        self.areas = areas.into_iter().collect();
        self
    }

    async fn send(self) -> Result<Story> {
        let client = self.client;
        let peer = client.business_user(&self.business_connection_id).await?;
        let (caption, entities) = client.write_text(self.caption, self.parse_mode).await?;
        let request = functions::stories::EditStory {
            peer,
            id: self.story_id,
            media: Some(self.content.upload(client).await?),
            media_areas: Some(StoryArea::write_all(&self.areas, client).await?),
            caption: Some(caption),
            entities: Some(entities),
            privacy_rules: Some(vec![base::InputPrivacyRule::InputPrivacyValueAllowAll]),
            music: None,
        };
        Story::from_updates(client, client.invoke(&request).await?)
    }
}

caption_setters!(EditBusinessStory);
into_future!(EditBusinessStory, Story);
