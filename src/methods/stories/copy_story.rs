use crate::Client;
use crate::crypto::random;
use crate::dispatcher::Entities;
use crate::enums::{ParseMode, StoriesPrivacyRules};
use crate::errors::{Error, Result};
use crate::methods::messages::caption_setters;
use crate::raw::{base, functions, types};
use crate::types::messages_and_media::{input_document_of, input_photo_of};
use crate::types::{PeerRef, Story};

/// Posts the story's file again by reference, without downloading it. Media areas and music aren't copied.
#[must_use = "a request does nothing until it is awaited"]
pub struct CopyStory<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    from_chat_id: PeerRef,
    story_id: i32,
    caption: Option<String>,
    parse_mode: Option<ParseMode>,
    privacy: StoriesPrivacyRules,
    period: Option<i32>,
    pinned: bool,
    protect_content: bool,
}

impl Client {
    pub fn copy_story(
        &self,
        chat_id: impl Into<PeerRef>,
        from_chat_id: impl Into<PeerRef>,
        story_id: i32,
    ) -> CopyStory<'_> {
        CopyStory {
            client: self,
            chat_id: chat_id.into(),
            from_chat_id: from_chat_id.into(),
            story_id,
            caption: None,
            parse_mode: None,
            privacy: StoriesPrivacyRules::default(),
            period: None,
            pinned: false,
            protect_content: false,
        }
    }
}

impl CopyStory<'_> {
    /// Replaces the original caption; an empty one removes it.
    pub fn caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    pub fn privacy(mut self, privacy: StoriesPrivacyRules) -> Self {
        self.privacy = privacy;
        self
    }

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

    async fn request(&self, item: types::StoryItem) -> Result<functions::stories::SendStory> {
        let (caption, entities) = match &self.caption {
            Some(text) => {
                let (caption, entities) = self.client.parse_text(text, self.parse_mode).await?;
                (caption, (!entities.is_empty()).then_some(entities))
            }
            None => (item.caption.unwrap_or_default(), input_entities(self.client, item.entities).await?),
        };
        Ok(functions::stories::SendStory {
            pinned: self.pinned,
            noforwards: self.protect_content,
            fwd_modified: false,
            peer: self.client.resolve_peer(self.chat_id.clone()).await?,
            media: story_media(item.media)?,
            media_areas: None,
            caption: (!caption.is_empty()).then_some(caption),
            entities: entities.filter(|e| !e.is_empty()),
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
        let peer = self.client.resolve_peer(self.from_chat_id.clone()).await?;
        let request = functions::stories::GetStoriesByID { peer, id: vec![self.story_id] };
        let base::stories::Stories::Stories(found) = self.client.invoke(&request).await?;
        let found = *found;
        // Kept mentions are sent back as input entities, which need these users' access hashes.
        Entities::new(self.client, found.users, found.chats);
        let item = found
            .stories
            .into_iter()
            .find_map(|s| match s {
                base::StoryItem::StoryItem(s) if s.id == self.story_id => Some(*s),
                _ => None,
            })
            .ok_or(Error::Unexpected("Telegram returned no story to copy"))?;
        let request = self.request(item).await?;
        Story::from_updates(self.client, self.client.invoke(&request).await?)
    }
}

caption_setters!(CopyStory);
into_future!(CopyStory, Story);

fn story_media(media: base::MessageMedia) -> Result<base::InputMedia> {
    let unsupported = Error::Unexpected("this story's media can't be copied");
    Ok(match media {
        base::MessageMedia::MessageMediaPhoto(m) => {
            let id = m.photo.and_then(input_photo_of).ok_or(unsupported)?;
            let photo =
                types::InputMediaPhoto { spoiler: false, live_photo: false, id, ttl_seconds: None, video: None };
            base::InputMedia::InputMediaPhoto(Box::new(photo))
        }
        base::MessageMedia::MessageMediaDocument(m) => {
            let document = types::InputMediaDocument {
                spoiler: false,
                id: m.document.and_then(input_document_of).ok_or(unsupported)?,
                video_cover: None,
                video_timestamp: None,
                ttl_seconds: None,
                query: None,
            };
            base::InputMedia::InputMediaDocument(Box::new(document))
        }
        _ => return Err(unsupported),
    })
}

/// Received mentions carry a bare user id; sending them needs the user's access hash.
async fn input_entities(
    client: &Client,
    mut entities: Option<Vec<base::MessageEntity>>,
) -> Result<Option<Vec<base::MessageEntity>>> {
    for entity in entities.iter_mut().flatten() {
        if let base::MessageEntity::MessageEntityMentionName(m) = entity {
            let user_id = client.resolve_user(&m.user_id.into()).await?;
            let mention = types::InputMessageEntityMentionName { offset: m.offset, length: m.length, user_id };
            *entity = base::MessageEntity::InputMessageEntityMentionName(Box::new(mention));
        }
    }
    Ok(entities)
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::functions::stories;
    use crate::raw::{Identifiable, Serializable, base, types};

    fn item(media: base::MessageMedia, entities: Vec<base::MessageEntity>) -> types::StoryItem {
        types::StoryItem {
            pinned: false,
            public: false,
            close_friends: false,
            min: false,
            noforwards: false,
            edited: false,
            contacts: false,
            selected_contacts: false,
            out: true,
            id: 5,
            date: 100,
            from_id: None,
            fwd_from: None,
            expire_date: 200,
            caption: Some("hi".into()),
            entities: Some(entities),
            media,
            media_areas: None,
            privacy: None,
            views: None,
            sent_reaction: None,
            albums: None,
            music: None,
        }
    }

    fn bold() -> base::MessageEntity {
        base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 2 })
    }

    fn photo() -> base::MessageMedia {
        let photo = types::Photo {
            has_stickers: false,
            id: 1,
            access_hash: 2,
            file_reference: vec![3],
            date: 4,
            sizes: Vec::new(),
            video_sizes: None,
            dc_id: 2,
        };
        let media = types::MessageMediaPhoto {
            spoiler: false,
            live_photo: false,
            photo: Some(base::Photo::Photo(Box::new(photo))),
            ttl_seconds: None,
            video: None,
        };
        base::MessageMedia::MessageMediaPhoto(Box::new(media))
    }

    fn video() -> base::MessageMedia {
        let document = types::Document {
            id: 1,
            access_hash: 2,
            file_reference: vec![3],
            date: 4,
            mime_type: "video/mp4".into(),
            size: 5,
            thumbs: None,
            video_thumbs: None,
            dc_id: 2,
            attributes: Vec::new(),
        };
        let media = types::MessageMediaDocument {
            nopremium: false,
            spoiler: false,
            video: true,
            round: false,
            voice: false,
            document: Some(base::Document::Document(Box::new(document))),
            alt_documents: None,
            video_cover: None,
            video_timestamp: None,
            ttl_seconds: None,
        };
        base::MessageMedia::MessageMediaDocument(Box::new(media))
    }

    #[tokio::test]
    async fn a_photo_story_is_resent_by_file_reference() {
        let client = Client::offline();
        let request = client.copy_story("me", "me", 5).request(item(photo(), vec![bold()])).await.unwrap();
        let id = types::InputPhoto { id: 1, access_hash: 2, file_reference: vec![3] };
        let media = types::InputMediaPhoto {
            spoiler: false,
            live_photo: false,
            id: base::InputPhoto::InputPhoto(Box::new(id)),
            ttl_seconds: None,
            video: None,
        };
        assert_eq!(request.media, base::InputMedia::InputMediaPhoto(Box::new(media)));
        assert_eq!((request.caption.as_deref(), request.entities), (Some("hi"), Some(vec![bold()])));
        assert_eq!((request.fwd_from_id, request.fwd_from_story), (None, None));
        assert_eq!(request.privacy_rules, [base::InputPrivacyRule::InputPrivacyValueAllowAll]);
        assert_eq!(request.peer, base::InputPeer::InputPeerSelf);
    }

    #[tokio::test]
    async fn a_video_story_reuses_its_document() {
        let client = Client::offline();
        let request = client.copy_story("me", "me", 5).request(item(video(), Vec::new())).await.unwrap();
        let base::InputMedia::InputMediaDocument(media) = request.media else { panic!("{:?}", request.media) };
        let id = types::InputDocument { id: 1, access_hash: 2, file_reference: vec![3] };
        assert_eq!(media.id, base::InputDocument::InputDocument(Box::new(id)));
        assert_eq!(request.entities, None);
    }

    #[tokio::test]
    async fn a_new_caption_replaces_the_original() {
        let client = Client::offline();
        let copy = client.copy_story("me", "me", 5).caption("<b>new</b>").html();
        let request = copy.request(item(photo(), Vec::new())).await.unwrap();
        let new_bold = base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 3 });
        assert_eq!((request.caption.as_deref(), request.entities), (Some("new"), Some(vec![new_bold])));

        let copy = client.copy_story("me", "me", 5).caption("");
        let request = copy.request(item(photo(), vec![bold()])).await.unwrap();
        assert_eq!((request.caption, request.entities), (None, None));
    }

    #[tokio::test]
    async fn kept_mentions_get_the_access_hash() {
        let mut buf = Vec::new();
        // A user with only access_hash set.
        for word in [1u32, 0] {
            word.serialize(&mut buf);
        }
        7i64.serialize(&mut buf);
        2i64.serialize(&mut buf);
        let user = base::User::User(Box::new(types::User::deserialize_bare(&mut &buf[..]).unwrap()));
        let mention = types::MessageEntityMentionName { offset: 0, length: 2, user_id: 7 };
        let mention = base::MessageEntity::MessageEntityMentionName(Box::new(mention));
        let found = types::stories::Stories {
            count: 1,
            stories: vec![base::StoryItem::StoryItem(Box::new(item(photo(), vec![mention])))],
            pinned_to_top: None,
            chats: Vec::new(),
            users: vec![user],
        };
        let reply = base::stories::Stories::Stories(Box::new(found)).to_bytes();
        let (client, mut rx) = Client::scripted(vec![Ok(reply)]);
        let _ = client.copy_story("me", "me", 5).await;
        let _ = rx.try_recv().unwrap();
        let body = rx.try_recv().unwrap();
        assert_eq!(u32::from_le_bytes(body[..4].try_into().unwrap()), stories::SendStory::CONSTRUCTOR_ID);
        let user_id = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 7, access_hash: 2 }));
        let expected = types::InputMessageEntityMentionName { offset: 0, length: 2, user_id };
        let expected = base::MessageEntity::InputMessageEntityMentionName(Box::new(expected)).to_bytes();
        assert!(body.windows(expected.len()).any(|w| w == expected.as_slice()));
    }

    #[tokio::test]
    async fn a_missing_story_is_an_error_and_sends_no_copy() {
        let found = types::stories::Stories {
            count: 1,
            stories: vec![base::StoryItem::StoryItemDeleted(types::StoryItemDeleted { id: 5 })],
            pinned_to_top: None,
            chats: Vec::new(),
            users: Vec::new(),
        };
        let reply = base::stories::Stories::Stories(Box::new(found)).to_bytes();
        let (client, mut rx) = Client::scripted(vec![Ok(reply)]);
        let result = client.copy_story("me", "me", 5).await;
        assert!(matches!(result, Err(Error::Unexpected(_))), "{result:?}");
        let expected = stories::GetStoriesByID { peer: base::InputPeer::InputPeerSelf, id: vec![5] };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn media_without_a_file_is_refused() {
        let client = Client::offline();
        let result =
            client.copy_story("me", "me", 5).request(item(base::MessageMedia::MessageMediaEmpty, vec![])).await;
        assert!(matches!(result, Err(Error::Unexpected(_))), "{result:?}");
    }
}
