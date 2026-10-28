use crate::Client;
use crate::crypto::random;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{PeerRef, Story};

#[must_use = "a request does nothing until it is awaited"]
pub struct RepostStory<'a> {
    client: &'a Client,
    business_connection_id: String,
    from_chat_id: PeerRef,
    from_story_id: i32,
    active_period: i32,
    post_to_chat_page: bool,
    protect_content: bool,
}

impl Client {
    /// Reposts a story from one business account to another; both must be connected to this bot.
    pub fn repost_story(
        &self,
        business_connection_id: impl Into<String>,
        from_chat_id: impl Into<PeerRef>,
        from_story_id: i32,
        active_period: i32,
    ) -> RepostStory<'_> {
        RepostStory {
            client: self,
            business_connection_id: business_connection_id.into(),
            from_chat_id: from_chat_id.into(),
            from_story_id,
            active_period,
            post_to_chat_page: false,
            protect_content: false,
        }
    }
}

impl RepostStory<'_> {
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
        let request = functions::stories::SendStory {
            pinned: self.post_to_chat_page,
            noforwards: self.protect_content,
            fwd_modified: false,
            peer: client.business_user(&self.business_connection_id).await?,
            media: base::InputMedia::InputMediaEmpty,
            media_areas: None,
            caption: None,
            entities: None,
            privacy_rules: vec![base::InputPrivacyRule::InputPrivacyValueAllowAll],
            random_id: i64::from_le_bytes(random()),
            period: Some(self.active_period),
            fwd_from_id: Some(client.resolve_peer(self.from_chat_id).await?),
            fwd_from_story: Some(self.from_story_id),
            albums: None,
            music: None,
        };
        Story::from_updates(client, client.invoke(&request).await?)
    }
}

into_future!(RepostStory, Story);
