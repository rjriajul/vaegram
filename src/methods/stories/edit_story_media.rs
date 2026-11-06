use crate::Client;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::methods::messages::caption_setters;
use crate::raw::{base, functions};
use crate::types::messages_and_media::input_document;
use crate::types::{FileLocation, InputStoryContent, PeerRef, Story, StoryArea, Text};

/// Replaces a story's media. Only what is set besides the media changes: areas, caption and music keep their
/// current value otherwise.
#[must_use = "a request does nothing until it is awaited"]
pub struct EditStoryMedia<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    story_id: i32,
    media: InputStoryContent<'a>,
    areas: Option<Vec<StoryArea>>,
    caption: Option<Text>,
    parse_mode: Option<ParseMode>,
    music: Option<String>,
}

impl Client {
    /// Replaces the media of a story you posted and returns the edited [`Story`].
    ///
    /// Awaiting it uploads the new media first. The caption and media areas stay as they were unless set.
    pub fn edit_story_media<'a>(
        &'a self,
        chat_id: impl Into<PeerRef>,
        story_id: i32,
        media: InputStoryContent<'a>,
    ) -> EditStoryMedia<'a> {
        EditStoryMedia {
            client: self,
            chat_id: chat_id.into(),
            story_id,
            media,
            areas: None,
            caption: None,
            parse_mode: None,
            music: None,
        }
    }
}

impl EditStoryMedia<'_> {
    /// Replaces the story's media areas. An empty list removes them.
    pub fn media_areas(mut self, areas: impl IntoIterator<Item = StoryArea>) -> Self {
        self.areas = Some(areas.into_iter().collect());
        self
    }

    /// Replaces the caption. An empty one removes it.
    pub fn caption(mut self, caption: impl Into<Text>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// Plays the audio file with this file id along with the story.
    ///
    /// Fails with `Error::InvalidArgument` if the file id doesn't name a document.
    pub fn music(mut self, file_id: impl Into<String>) -> Self {
        self.music = Some(file_id.into());
        self
    }

    async fn request(
        self,
        media: base::InputMedia,
        music: Option<base::InputDocument>,
    ) -> Result<functions::stories::EditStory> {
        let peer = self.client.resolve_peer(self.chat_id.clone()).await?;
        let media_areas = match &self.areas {
            Some(areas) => Some(StoryArea::write_all(areas, self.client).await?),
            None => None,
        };
        let (caption, entities) = match self.caption {
            Some(text) => {
                let (caption, entities) = self.client.write_text(text, self.parse_mode).await?;
                (Some(caption), Some(entities))
            }
            None => (None, None),
        };
        Ok(functions::stories::EditStory {
            peer,
            id: self.story_id,
            media: Some(media),
            media_areas,
            caption,
            entities,
            privacy_rules: None,
            music,
        })
    }

    async fn send(self) -> Result<Story> {
        let client = self.client;
        let music = self.music.as_deref().map(story_music).transpose()?;
        let media = self.media.upload(client).await?;
        let request = self.request(media, music).await?;
        Story::from_updates(client, client.invoke(&request).await?)
    }
}

caption_setters!(EditStoryMedia);
into_future!(EditStoryMedia, Story);

/// The document a story's music file id names. Anything else is refused before a request goes out.
pub(crate) fn story_music(file_id: &str) -> Result<base::InputDocument> {
    let location = FileLocation::try_from(file_id)?;
    // A thumbnail id names a document file location too, but of the document's preview, not the file.
    let is_thumbnail = matches!(
        &location.location,
        base::InputFileLocation::InputDocumentFileLocation(l) if !l.thumb_size.is_empty()
    );
    match input_document(&location) {
        document @ base::InputDocument::InputDocument(_) if !is_thumbnail => Ok(document),
        _ => Err(Error::InvalidArgument("story music must be a document file id")),
    }
}

#[cfg(test)]
mod tests {
    use super::story_music;
    use crate::Client;
    use crate::errors::Error;
    use crate::file_id::{FileId, FileType, ThumbnailSource};
    use crate::raw::{base, types};
    use crate::types::{InputStoryContent, StoryArea, StoryAreaPosition, StoryAreaType, Text};

    fn photo_id() -> String {
        FileId::new(FileType::Photo, 2, 5, 6, &[1]).encode()
    }

    fn document_id() -> String {
        FileId::new(FileType::Audio, 2, 7, 8, &[9]).encode()
    }

    fn thumbnail_id() -> String {
        let source = ThumbnailSource::Thumbnail { file_type: FileType::Audio, size: 'm' };
        FileId { thumbnail_source: Some(source), ..FileId::new(FileType::Thumbnail, 2, 7, 8, &[9]) }.encode()
    }

    #[tokio::test]
    async fn edit_media_sends_only_set_fields() {
        let client = Client::offline();
        let plain = client.edit_story_media("me", 4, InputStoryContent::photo("a.jpg"));
        let request = plain.request(base::InputMedia::InputMediaEmpty, None).await.unwrap();
        assert_eq!(request.id, 4);
        assert_eq!(request.media, Some(base::InputMedia::InputMediaEmpty));
        assert_eq!(
            (request.media_areas, request.caption, request.entities, request.privacy_rules, request.music),
            (None, None, None, None, None)
        );

        let music = story_music(&document_id()).unwrap();
        let edited = client.edit_story_media("me", 4, InputStoryContent::photo("a.jpg")).caption("<b>hi</b>").html();
        let request = edited.request(base::InputMedia::InputMediaEmpty, Some(music.clone())).await.unwrap();
        assert_eq!(request.caption.as_deref(), Some("hi"));
        assert!(matches!(request.entities.as_deref(), Some([base::MessageEntity::MessageEntityBold(_)])));
        assert_eq!(request.music, Some(music));

        let caption = Text::new().bold("hi");
        let built = client.edit_story_media("me", 4, InputStoryContent::photo("a.jpg")).caption(caption);
        let request = built.request(base::InputMedia::InputMediaEmpty, None).await.unwrap();
        let bold = base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 2 });
        assert_eq!((request.caption.as_deref(), request.entities), (Some("hi"), Some(vec![bold])));

        let cleared = client.edit_story_media("me", 4, InputStoryContent::photo("a.jpg")).caption("");
        let request = cleared.request(base::InputMedia::InputMediaEmpty, None).await.unwrap();
        assert_eq!((request.caption.as_deref(), request.entities), (Some(""), Some(Vec::new())));
    }

    #[tokio::test]
    async fn edit_media_writes_areas_through_the_client() {
        let client = Client::offline();
        let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        let chat = crate::utils::get_channel_id(9);
        client.0.peers.lock().unwrap().insert(chat, base::InputPeer::InputPeerChannel(Box::new(peer)));
        let post = StoryArea::new(
            StoryAreaPosition::default(),
            StoryAreaType::ChannelPost {
                chat: Box::new(crate::types::Chat::parse(
                    &crate::utils::get_peer(chat),
                    &crate::dispatcher::Entities::default(),
                    true,
                )),
                message_id: 3,
            },
        );
        let expected = post.write(&client).await.unwrap();
        assert!(matches!(expected, base::MediaArea::InputMediaAreaChannelPost(_)));
        let edit = client.edit_story_media("me", 4, InputStoryContent::photo("a.jpg")).media_areas([post]);
        let request = edit.request(base::InputMedia::InputMediaEmpty, None).await.unwrap();
        assert_eq!(request.media_areas, Some(vec![expected]));

        let clear = client.edit_story_media("me", 4, InputStoryContent::photo("a.jpg")).media_areas([]);
        let request = clear.request(base::InputMedia::InputMediaEmpty, None).await.unwrap();
        assert_eq!(request.media_areas, Some(Vec::new()));
    }

    #[test]
    fn music_takes_a_document_file_id() {
        let base::InputDocument::InputDocument(d) = story_music(&document_id()).unwrap() else {
            panic!("expected a document")
        };
        assert_eq!((d.id, d.access_hash, d.file_reference), (7, 8, vec![9]));
    }

    #[tokio::test]
    async fn non_document_music_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        for music in [photo_id(), thumbnail_id(), "not a file id".to_owned()] {
            assert!(matches!(story_music(&music), Err(Error::InvalidArgument(_))), "{music}");
            let edit = client.edit_story_media("me", 4, InputStoryContent::photo("a.jpg")).music(music);
            assert!(matches!(edit.await, Err(Error::InvalidArgument(_))));
        }
        assert!(rx.try_recv().is_err());
    }
}
