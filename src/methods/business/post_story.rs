use crate::Client;
use crate::crypto::random;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::methods::messages::caption_setters;
use crate::raw::{base, functions};
use crate::types::{InputStoryContent, Story, StoryArea};

#[must_use = "a request does nothing until it is awaited"]
pub struct PostStory<'a> {
    client: &'a Client,
    business_connection_id: String,
    content: InputStoryContent<'a>,
    active_period: i32,
    caption: String,
    parse_mode: Option<ParseMode>,
    areas: Vec<StoryArea>,
    post_to_chat_page: bool,
    protect_content: bool,
}

impl Client {
    /// Posts a story for a business account; `active_period` is in seconds: 6, 12, 24 or 48 hours.
    pub fn post_story<'a>(
        &'a self,
        business_connection_id: impl Into<String>,
        content: InputStoryContent<'a>,
        active_period: i32,
    ) -> PostStory<'a> {
        PostStory {
            client: self,
            business_connection_id: business_connection_id.into(),
            content,
            active_period,
            caption: String::new(),
            parse_mode: None,
            areas: Vec::new(),
            post_to_chat_page: false,
            protect_content: false,
        }
    }

    // Business stories go straight to the account's own user peer; only deleting one needs the connection.
    pub(crate) async fn business_user(&self, business_connection_id: &str) -> Result<base::InputPeer> {
        let connection = self.get_business_connection(business_connection_id).await?;
        self.resolve_peer(connection.user.id).await
    }
}

impl PostStory<'_> {
    pub fn caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = caption.into();
        self
    }

    pub fn areas(mut self, areas: impl IntoIterator<Item = StoryArea>) -> Self {
        self.areas = areas.into_iter().collect();
        self
    }

    pub fn post_to_chat_page(mut self) -> Self {
        self.post_to_chat_page = true;
        self
    }

    pub fn protect_content(mut self) -> Self {
        self.protect_content = true;
        self
    }

    async fn send(self) -> Result<Story> {
        let client = self.client;
        let peer = client.business_user(&self.business_connection_id).await?;
        let (caption, entities) = client.parse_text(&self.caption, self.parse_mode).await?;
        let request = functions::stories::SendStory {
            pinned: self.post_to_chat_page,
            noforwards: self.protect_content,
            fwd_modified: false,
            peer,
            media: self.content.upload(client).await?,
            media_areas: Some(self.areas.iter().map(StoryArea::write).collect()),
            caption: Some(caption),
            entities: Some(entities),
            privacy_rules: vec![base::InputPrivacyRule::InputPrivacyValueAllowAll],
            random_id: i64::from_le_bytes(random()),
            period: Some(self.active_period),
            fwd_from_id: None,
            fwd_from_story: None,
            albums: None,
            music: None,
        };
        Story::from_updates(client, client.invoke(&request).await?)
    }
}

caption_setters!(PostStory);
into_future!(PostStory, Story);
