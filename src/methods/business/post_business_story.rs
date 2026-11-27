use crate::Client;
use crate::crypto::random;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::methods::messages::caption_setters;
use crate::raw::{base, functions};
use crate::types::{InputStoryContent, Story, StoryArea, Text};

/// Request built by [`Client::post_business_story`]. Awaiting it returns the posted [`Story`].
#[must_use = "a request does nothing until it is awaited"]
pub struct PostBusinessStory<'a> {
    client: &'a Client,
    business_connection_id: String,
    content: InputStoryContent<'a>,
    active_period: i32,
    caption: Text,
    parse_mode: Option<ParseMode>,
    areas: Vec<StoryArea>,
    post_to_chat_page: bool,
    protect_content: bool,
}

impl Client {
    /// Posts a story for the business account behind a connection, visible to everyone.
    ///
    /// `active_period` is in seconds and must be 6, 12, 24 or 48 hours.
    ///
    /// In Bot API mode (`ClientBuilder::bot_api`) the returned story holds its chat, id, dates, caption text and
    /// flags, but no media: the Bot API answers with the story's chat and id alone.
    pub fn post_business_story<'a>(
        &'a self,
        business_connection_id: impl Into<String>,
        content: InputStoryContent<'a>,
        active_period: i32,
    ) -> PostBusinessStory<'a> {
        PostBusinessStory {
            client: self,
            business_connection_id: business_connection_id.into(),
            content,
            active_period,
            caption: Text::new(),
            parse_mode: None,
            areas: Vec::new(),
            post_to_chat_page: false,
            protect_content: false,
        }
    }

    // Business stories go straight to the account's own user peer; only deleting one needs the connection.
    pub(crate) async fn business_user(&self, business_connection_id: &str) -> Result<base::InputPeer> {
        let user_id = self.business_home(business_connection_id).await?.user_id;
        self.resolve_peer(user_id).await
    }
}

impl PostBusinessStory<'_> {
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

    /// Keeps the story on the account's profile after it expires.
    pub fn post_to_chat_page(mut self) -> Self {
        self.post_to_chat_page = true;
        self
    }

    /// Stops viewers from forwarding or saving the story.
    pub fn protect_content(mut self) -> Self {
        self.protect_content = true;
        self
    }

    async fn send(self) -> Result<Story> {
        let client = self.client;
        // In Bot API mode the file goes in the `postStory` request, which names the connection, not the account.
        #[cfg(feature = "bot-api")]
        if let Some(api) = client.0.bot_api.as_deref() {
            let (caption, entities) = client.write_text(self.caption, self.parse_mode).await?;
            let story = (self.content, caption, entities, self.areas.as_slice());
            let flags = (self.post_to_chat_page, self.protect_content);
            return api.post_story(client, &self.business_connection_id, story, self.active_period, flags).await;
        }
        let peer = client.business_user(&self.business_connection_id).await?;
        let (caption, entities) = client.write_text(self.caption, self.parse_mode).await?;
        let request = functions::stories::SendStory {
            pinned: self.post_to_chat_page,
            noforwards: self.protect_content,
            fwd_modified: false,
            peer,
            media: self.content.upload(client).await?,
            media_areas: Some(StoryArea::write_all(&self.areas, client).await?),
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

caption_setters!(PostBusinessStory);
into_future!(PostBusinessStory, Story);
