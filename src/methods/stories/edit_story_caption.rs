use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::methods::messages::caption_setters;
use crate::raw::functions;
use crate::types::{PeerRef, Story};

#[must_use = "a request does nothing until it is awaited"]
pub struct EditStoryCaption<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    story_id: i32,
    caption: String,
    parse_mode: Option<ParseMode>,
}

impl Client {
    /// An empty caption removes it.
    pub fn edit_story_caption(
        &self,
        chat_id: impl Into<PeerRef>,
        story_id: i32,
        caption: impl Into<String>,
    ) -> EditStoryCaption<'_> {
        EditStoryCaption { client: self, chat_id: chat_id.into(), story_id, caption: caption.into(), parse_mode: None }
    }
}

impl EditStoryCaption<'_> {
    async fn send(self) -> Result<Story> {
        let peer = self.client.resolve_peer(self.chat_id).await?;
        let (caption, entities) = self.client.parse_text(&self.caption, self.parse_mode).await?;
        let request = functions::stories::EditStory {
            peer,
            id: self.story_id,
            media: None,
            media_areas: None,
            caption: Some(caption),
            entities: Some(entities),
            privacy_rules: None,
            music: None,
        };
        Story::from_updates(self.client, self.client.invoke(&request).await?)
    }
}

caption_setters!(EditStoryCaption);
into_future!(EditStoryCaption, Story);
