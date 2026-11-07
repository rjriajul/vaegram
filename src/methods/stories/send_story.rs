use super::edit_story_media::story_music;
use crate::Client;
use crate::crypto::random;
use crate::enums::{ParseMode, StoriesPrivacyRules};
use crate::errors::Result;
use crate::methods::messages::caption_setters;
use crate::raw::{base, functions};
use crate::types::{InputStoryContent, PeerRef, Story, StoryArea, Text};

/// Request built by [`Client::send_story`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SendStory<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    media: InputStoryContent<'a>,
    caption: Text,
    parse_mode: Option<ParseMode>,
    privacy: StoriesPrivacyRules,
    period: Option<i32>,
    pinned: bool,
    protect_content: bool,
    areas: Vec<StoryArea>,
    music: Option<String>,
    albums: Option<Vec<i32>>,
    repost: Option<(PeerRef, i32)>,
    repost_modified: bool,
}

impl Client {
    /// Posts a story to a chat and returns the new [`Story`].
    ///
    /// Awaiting it uploads the media first, then publishes the story.
    pub fn send_story<'a>(&'a self, chat_id: impl Into<PeerRef>, media: InputStoryContent<'a>) -> SendStory<'a> {
        SendStory {
            client: self,
            chat_id: chat_id.into(),
            media,
            caption: Text::new(),
            parse_mode: None,
            privacy: StoriesPrivacyRules::default(),
            period: None,
            pinned: false,
            protect_content: false,
            areas: Vec::new(),
            music: None,
            albums: None,
            repost: None,
            repost_modified: false,
        }
    }
}

impl SendStory<'_> {
    /// Adds a caption, parsed with the builder's parse mode.
    ///
    /// Telegram's `stories_entities` setting decides who may format a story caption; while it is `premium`, a
    /// caption with entities from an account without Premium fails with `PREMIUM_ACCOUNT_REQUIRED`.
    pub fn caption(mut self, caption: impl Into<Text>) -> Self {
        self.caption = caption.into();
        self
    }

    /// Chooses who can see the story.
    pub fn privacy(mut self, privacy: StoriesPrivacyRules) -> Self {
        self.privacy = privacy;
        self
    }

    /// Keeps the story up for this many seconds.
    ///
    /// Premium accounts can pick 6, 12, 24 or 48 hours. Without Premium, Telegram accepts only 24 hours.
    pub fn period(mut self, seconds: i32) -> Self {
        self.period = Some(seconds);
        self
    }

    /// Pins the story to the chat's profile.
    pub fn pinned(mut self) -> Self {
        self.pinned = true;
        self
    }

    /// Attaches interactive areas to the story, such as locations or reactions.
    pub fn media_areas(mut self, areas: impl IntoIterator<Item = StoryArea>) -> Self {
        self.areas = areas.into_iter().collect();
        self
    }

    /// Stops viewers from forwarding or saving the story.
    pub fn protect_content(mut self) -> Self {
        self.protect_content = true;
        self
    }

    /// Plays the audio file with this file id along with the story.
    ///
    /// Fails with `Error::InvalidArgument` if the file id doesn't name a document.
    pub fn music(mut self, file_id: impl Into<String>) -> Self {
        self.music = Some(file_id.into());
        self
    }

    /// Adds the story to the albums with these ids.
    pub fn albums(mut self, album_ids: impl IntoIterator<Item = i32>) -> Self {
        self.albums = Some(album_ids.into_iter().collect());
        self
    }

    /// Posts the story as a repost of another chat's story.
    pub fn fwd_from(mut self, chat_id: impl Into<PeerRef>, story_id: i32) -> Self {
        self.repost = Some((chat_id.into(), story_id));
        self
    }

    /// Marks a repost as changed from the original.
    pub fn fwd_modified(mut self) -> Self {
        self.repost_modified = true;
        self
    }

    async fn request(self, media: base::InputMedia) -> Result<functions::stories::SendStory> {
        let music = self.music.as_deref().map(story_music).transpose()?;
        let peer = self.client.resolve_peer(self.chat_id.clone()).await?;
        let (caption, entities) = self.client.write_text(self.caption, self.parse_mode).await?;
        let areas = StoryArea::write_all(&self.areas, self.client).await?;
        let (fwd_from_id, fwd_from_story) = match &self.repost {
            Some((chat_id, story_id)) => (Some(self.client.resolve_peer(chat_id.clone()).await?), Some(*story_id)),
            None => (None, None),
        };
        Ok(functions::stories::SendStory {
            pinned: self.pinned,
            noforwards: self.protect_content,
            fwd_modified: self.repost_modified,
            peer,
            media,
            media_areas: (!self.areas.is_empty()).then_some(areas),
            caption: (!caption.is_empty()).then_some(caption),
            entities: (!entities.is_empty()).then_some(entities),
            privacy_rules: self.client.input_privacy_rules(&self.privacy.rules()).await?,
            random_id: i64::from_le_bytes(random()),
            period: self.period,
            fwd_from_id,
            fwd_from_story,
            albums: self.albums.clone(),
            music,
        })
    }

    async fn send(self) -> Result<Story> {
        let client = self.client;
        // A bad music id is refused before the media is uploaded.
        self.music.as_deref().map(story_music).transpose()?;
        let media = self.media.upload(client).await?;
        let request = self.request(media).await?;
        Story::from_updates(client, client.invoke(&request).await?)
    }
}

caption_setters!(SendStory);
into_future!(SendStory, Story);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::{FileType, StoriesPrivacyRules};
    use crate::errors::Error;
    use crate::file_id::FileId;
    use crate::raw::{base, types};
    use crate::types::{InputStoryContent, Text};
    use crate::types::{StoryArea, StoryAreaPosition, StoryAreaType};

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

        let built = client.send_story("me", InputStoryContent::photo("story.jpg")).caption(Text::new().bold("hi"));
        let request = built.request(base::InputMedia::InputMediaEmpty).await.unwrap();
        let bold = base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 2 });
        assert_eq!((request.caption.as_deref(), request.entities), (Some("hi"), Some(vec![bold])));
    }

    #[tokio::test]
    async fn sends_media_areas() {
        let client = Client::offline();
        let position = StoryAreaPosition::new(50.0, 50.0, 0.0, 0.0);
        let link = StoryArea::new(position, StoryAreaType::Link { url: "https://example.com".into() });
        let story = client.send_story("me", InputStoryContent::photo("story.jpg")).media_areas([link.clone()]);
        let request = story.request(base::InputMedia::InputMediaEmpty).await.unwrap();
        let written = link.write(&client).await.unwrap();
        assert_eq!(request.media_areas, Some(vec![written]));

        let plain = client.send_story("me", InputStoryContent::photo("story.jpg"));
        let request = plain.request(base::InputMedia::InputMediaEmpty).await.unwrap();
        assert_eq!(request.media_areas, None);
    }

    #[tokio::test]
    async fn send_story_sends_music_albums_and_fwd() {
        let client = Client::offline();
        let music = FileId::new(FileType::Audio, 2, 7, 8, &[9]).encode();
        let story = client
            .send_story("me", InputStoryContent::photo("story.jpg"))
            .music(music)
            .albums([3, 4])
            .fwd_from("me", 9)
            .fwd_modified();
        let request = story.request(base::InputMedia::InputMediaEmpty).await.unwrap();
        let document = types::InputDocument { id: 7, access_hash: 8, file_reference: vec![9] };
        assert_eq!(request.music, Some(base::InputDocument::InputDocument(Box::new(document))));
        assert_eq!(request.albums, Some(vec![3, 4]));
        assert_eq!((request.fwd_from_id, request.fwd_from_story), (Some(base::InputPeer::InputPeerSelf), Some(9)));
        assert!(request.fwd_modified);

        let plain = client.send_story("me", InputStoryContent::photo("story.jpg"));
        let request = plain.request(base::InputMedia::InputMediaEmpty).await.unwrap();
        assert_eq!(
            (request.music, request.albums, request.fwd_from_id, request.fwd_from_story),
            (None, None, None, None)
        );
        assert!(!request.fwd_modified);

        let bad = client.send_story("me", InputStoryContent::photo("story.jpg")).music("not a file id");
        assert!(matches!(bad.request(base::InputMedia::InputMediaEmpty).await, Err(Error::InvalidArgument(_))));
    }
}
