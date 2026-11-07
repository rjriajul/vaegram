use crate::methods::{
    SendAnimation, SendAudio, SendCachedMedia, SendMediaGroup, SendMessage, SendPhoto, SendSticker, SendVideo,
    SendVideoNote, SendVoice,
};
use crate::types::{FileSource, InputMedia, Story, Text};

impl Story {
    /// Sends `text` to this story's chat as a reply to the story.
    pub fn reply(&self, text: impl Into<Text>) -> SendMessage<'_> {
        self.client().send_message(self.chat.id, text).reply_to(self)
    }

    /// Sends an animation to this story's chat as a reply to the story.
    pub fn reply_animation<'a>(&'a self, animation: impl Into<FileSource<'a>>) -> SendAnimation<'a> {
        self.client().send_animation(self.chat.id, animation).reply_to(self)
    }

    /// Sends an audio file to this story's chat as a reply to the story.
    pub fn reply_audio<'a>(&'a self, audio: impl Into<FileSource<'a>>) -> SendAudio<'a> {
        self.client().send_audio(self.chat.id, audio).reply_to(self)
    }

    /// Sends a file by its file id to this story's chat as a reply to the story.
    pub fn reply_cached_media(&self, file_id: impl Into<String>) -> SendCachedMedia<'_> {
        self.client().send_cached_media(self.chat.id, file_id).reply_to(self)
    }

    /// Sends an album to this story's chat as a reply to the story.
    pub fn reply_media_group<'a>(&'a self, media: impl IntoIterator<Item = InputMedia<'a>>) -> SendMediaGroup<'a> {
        self.client().send_media_group(self.chat.id, media).reply_to(self)
    }

    /// Sends a photo to this story's chat as a reply to the story.
    pub fn reply_photo<'a>(&'a self, photo: impl Into<FileSource<'a>>) -> SendPhoto<'a> {
        self.client().send_photo(self.chat.id, photo).reply_to(self)
    }

    /// Sends a sticker to this story's chat as a reply to the story.
    pub fn reply_sticker<'a>(&'a self, sticker: impl Into<FileSource<'a>>) -> SendSticker<'a> {
        self.client().send_sticker(self.chat.id, sticker).reply_to(self)
    }

    /// Sends a video to this story's chat as a reply to the story.
    pub fn reply_video<'a>(&'a self, video: impl Into<FileSource<'a>>) -> SendVideo<'a> {
        self.client().send_video(self.chat.id, video).reply_to(self)
    }

    /// Sends a video note to this story's chat as a reply to the story.
    pub fn reply_video_note<'a>(&'a self, video_note: impl Into<FileSource<'a>>) -> SendVideoNote<'a> {
        self.client().send_video_note(self.chat.id, video_note).reply_to(self)
    }

    /// Sends a voice message to this story's chat as a reply to the story.
    pub fn reply_voice<'a>(&'a self, voice: impl Into<FileSource<'a>>) -> SendVoice<'a> {
        self.client().send_voice(self.chat.id, voice).reply_to(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::enums::FileType;
    use crate::file_id::FileId;
    use crate::raw::{Serializable, base, types};
    use crate::types::{FileSource, InputMedia, Story};
    use crate::utils::get_peer;

    fn story(client: &Client) -> Story {
        let item = types::StoryItemSkipped { close_friends: false, live: false, id: 8, date: 1, expire_date: 2 };
        Story::parse(client, &get_peer(7), base::StoryItem::StoryItemSkipped(Box::new(item)), &Entities::default())
            .unwrap()
    }

    #[tokio::test]
    async fn story_replies_carry_input_reply_to_story() {
        let (client, mut rx) = Client::recording();
        let user = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 1 }));
        client.0.peers.lock().unwrap().insert(7, user.clone());
        let story = story(&client);
        let reply =
            base::InputReplyTo::InputReplyToStory(Box::new(types::InputReplyToStory { peer: user, story_id: 8 }))
                .to_bytes();
        let document = || FileSource::file_id(&FileId::new(FileType::Document, 1, 42, 7, &[]).encode()).unwrap();
        let _ = story.reply("hi").await;
        let _ = story.reply_photo(FileSource::url("https://example.com/a.jpg").unwrap()).await;
        let _ = story.reply_media_group([InputMedia::document(document()), InputMedia::document(document())]).await;
        for _ in 0..3 {
            let body = rx.try_recv().expect("no request sent");
            assert!(body.windows(reply.len()).any(|w| w == reply), "no inputReplyToStory");
        }
        assert!(rx.try_recv().is_err());
    }
}
