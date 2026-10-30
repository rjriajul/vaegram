use crate::Client;
use crate::crypto::random;
use crate::enums::{ParseMode, StoriesPrivacyRules};
use crate::errors::Result;
use crate::methods::messages::caption_setters;
use crate::raw::{base, functions};
use crate::types::{InputStoryContent, PeerRef, Story};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendStory<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    media: InputStoryContent<'a>,
    caption: String,
    parse_mode: Option<ParseMode>,
    privacy: StoriesPrivacyRules,
    period: Option<i32>,
    pinned: bool,
    protect_content: bool,
}

impl Client {
    pub fn send_story<'a>(&'a self, chat_id: impl Into<PeerRef>, media: InputStoryContent<'a>) -> SendStory<'a> {
        SendStory {
            client: self,
            chat_id: chat_id.into(),
            media,
            caption: String::new(),
            parse_mode: None,
            privacy: StoriesPrivacyRules::default(),
            period: None,
            pinned: false,
            protect_content: false,
        }
    }
}

impl SendStory<'_> {
    pub fn caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = caption.into();
        self
    }

    pub fn privacy(mut self, privacy: StoriesPrivacyRules) -> Self {
        self.privacy = privacy;
        self
    }

    /// 6, 12, 24 or 48 hours with Premium; otherwise Telegram only accepts 24 hours.
    pub fn period(mut self, seconds: i32) -> Self {
        self.period = Some(seconds);
        self
    }

    pub fn pinned(mut self) -> Self {
        self.pinned = true;
        self
    }

    pub fn protect_content(mut self) -> Self {
        self.protect_content = true;
        self
    }

    async fn request(&self, media: base::InputMedia) -> Result<functions::stories::SendStory> {
        let peer = self.client.resolve_peer(self.chat_id.clone()).await?;
        let (caption, entities) = self.client.parse_text(&self.caption, self.parse_mode).await?;
        Ok(functions::stories::SendStory {
            pinned: self.pinned,
            noforwards: self.protect_content,
            fwd_modified: false,
            peer,
            media,
            media_areas: None,
            caption: (!caption.is_empty()).then_some(caption),
            entities: (!entities.is_empty()).then_some(entities),
            privacy_rules: self.client.input_privacy_rules(&self.privacy.rules()).await?,
            random_id: i64::from_le_bytes(random()),
            period: self.period,
            fwd_from_id: None,
            fwd_from_story: None,
            albums: None,
            music: None,
        })
    }

    async fn send(self) -> Result<Story> {
        let media = self.media.upload(self.client).await?;
        let request = self.request(media).await?;
        Story::from_updates(self.client, self.client.invoke(&request).await?)
    }
}

caption_setters!(SendStory);
into_future!(SendStory, Story);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::StoriesPrivacyRules;
    use crate::raw::base;
    use crate::types::InputStoryContent;

    #[tokio::test]
    async fn request_carries_caption_privacy_and_flags() {
        let client = Client::offline();
        let story = client
            .send_story("me", InputStoryContent::photo("story.jpg"))
            .caption("<b>hi</b>")
            .html()
            .privacy(StoriesPrivacyRules::CloseFriends)
            .period(6 * 3600)
            .pinned()
            .protect_content();
        let request = story.request(base::InputMedia::InputMediaEmpty).await.unwrap();
        assert_eq!(request.peer, base::InputPeer::InputPeerSelf);
        assert_eq!(request.caption.as_deref(), Some("hi"));
        assert!(matches!(request.entities.as_deref(), Some([base::MessageEntity::MessageEntityBold(_)])));
        assert_eq!(request.privacy_rules, [base::InputPrivacyRule::InputPrivacyValueAllowCloseFriends]);
        assert_eq!(request.period, Some(6 * 3600));
        assert!(request.pinned && request.noforwards);

        let plain = client.send_story("me", InputStoryContent::photo("story.jpg"));
        let request = plain.request(base::InputMedia::InputMediaEmpty).await.unwrap();
        assert_eq!((request.caption, request.entities, request.period), (None, None, None));
        assert_eq!(request.privacy_rules, [base::InputPrivacyRule::InputPrivacyValueAllowAll]);
        assert!(!request.pinned && !request.noforwards);
    }
}
