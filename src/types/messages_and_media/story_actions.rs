use crate::enums::StoriesPrivacyRules;
use crate::errors::Result;
use crate::methods::{CopyStory, EditStoryCaption, EditStoryMedia, EditStoryPrivacy, ForwardStory, SendStoryReaction};
use crate::types::{InputStoryContent, PeerRef, ReactionType, Story, Text};

impl Story {
    /// Posts this story's file to `chat_id` again.
    pub fn copy(&self, chat_id: impl Into<PeerRef>) -> CopyStory<'_> {
        self.client().copy_story(chat_id, self.chat.id, self.id)
    }

    /// Deletes this story.
    pub async fn delete(&self) -> Result<()> {
        self.client().delete_stories(self.chat.id, &[self.id]).await?;
        Ok(())
    }

    /// Replaces the story's media. Awaiting it returns the edited [`Story`].
    ///
    /// The caption, media areas and music stay as they were unless set on the returned [`EditStoryMedia`].
    pub fn edit_media<'a>(&'a self, media: InputStoryContent<'a>) -> EditStoryMedia<'a> {
        self.client().edit_story_media(self.chat.id, self.id, media)
    }

    /// Edits the story's caption. An empty caption removes it.
    pub fn edit_caption(&self, caption: impl Into<Text>) -> EditStoryCaption<'_> {
        self.client().edit_story_caption(self.chat.id, self.id, caption)
    }

    /// Changes who can see the story. Awaiting it returns the edited [`Story`].
    pub fn edit_privacy(&self, privacy: StoriesPrivacyRules) -> EditStoryPrivacy<'_> {
        self.client().edit_story_privacy(self.chat.id, self.id, privacy)
    }

    /// Reacts to the story, replacing the account's earlier reaction.
    pub fn react(&self, reaction: impl Into<ReactionType>) -> SendStoryReaction<'_> {
        self.client().send_story_reaction(self.chat.id, self.id).reaction(reaction)
    }

    /// Removes the account's reaction from the story.
    pub fn unreact(&self) -> SendStoryReaction<'_> {
        self.client().send_story_reaction(self.chat.id, self.id)
    }

    /// Sends this story to `chat_id` as a message.
    pub fn forward(&self, chat_id: impl Into<PeerRef>) -> ForwardStory<'_> {
        self.client().forward_story(chat_id, self.chat.id, self.id)
    }

    /// Marks this story and the older ones of its chat as read. Returns the ids Telegram marked.
    pub async fn read(&self) -> Result<Vec<i32>> {
        self.client().read_stories(self.chat.id, self.id).await
    }

    /// Counts a view of this story.
    pub async fn view(&self) -> Result<()> {
        self.client().view_stories(self.chat.id, &[self.id]).await
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::enums::StoriesPrivacyRules;
    use crate::raw::functions::{messages, stories, upload};
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::types::{InputStoryContent, ReactionType, Story, Text, Upload};
    use crate::utils::get_peer;

    fn edit() -> stories::EditStory {
        stories::EditStory {
            peer: base::InputPeer::InputPeerSelf,
            id: 7,
            media: None,
            media_areas: None,
            caption: None,
            entities: None,
            privacy_rules: None,
            music: None,
        }
    }

    // Chat id 0 is the own account, so every helper's peer resolves without a lookup.
    #[tokio::test]
    async fn each_helper_sends_its_request() {
        let (client, mut rx) = Client::recording();
        let story = Story::from_media(&client, &get_peer(0), 7, &Entities::default());
        let me = base::InputPeer::InputPeerSelf;

        let _ = story.copy(0_i64).await;
        let fetch = stories::GetStoriesByID { peer: me.clone(), id: vec![7] };
        assert_eq!(rx.try_recv().unwrap(), fetch.to_bytes());

        let _ = story.delete().await;
        assert_eq!(rx.try_recv().unwrap(), stories::DeleteStories { peer: me.clone(), id: vec![7] }.to_bytes());

        let _ = story.edit_media(InputStoryContent::photo(Upload::bytes("a.jpg", b"x"))).await;
        assert_eq!(rx.try_recv().unwrap()[..4], upload::SaveFilePart::CONSTRUCTOR_ID.to_le_bytes());

        let _ = story.edit_caption("<i>new</i>").html().await;
        let italic = types::MessageEntityItalic { offset: 0, length: 3 };
        let expected = stories::EditStory {
            caption: Some("new".into()),
            entities: Some(vec![base::MessageEntity::MessageEntityItalic(italic)]),
            ..edit()
        };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());

        let _ = story.edit_caption(Text::new().bold("new")).await;
        let bold = types::MessageEntityBold { offset: 0, length: 3 };
        let expected = stories::EditStory {
            caption: Some("new".into()),
            entities: Some(vec![base::MessageEntity::MessageEntityBold(bold)]),
            ..edit()
        };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());

        let _ = story.edit_privacy(StoriesPrivacyRules::CloseFriends).await;
        let rules = vec![base::InputPrivacyRule::InputPrivacyValueAllowCloseFriends];
        assert_eq!(rx.try_recv().unwrap(), stories::EditStory { privacy_rules: Some(rules), ..edit() }.to_bytes());

        let _ = story.react(ReactionType::Emoji("🔥".into())).await;
        let fire = base::Reaction::ReactionEmoji(Box::new(types::ReactionEmoji { emoticon: "🔥".into() }));
        let react = stories::SendReaction { add_to_recent: true, peer: me.clone(), story_id: 7, reaction: fire };
        assert_eq!(rx.try_recv().unwrap(), react.to_bytes());

        let _ = story.react("👍").await;
        let thumb = base::Reaction::ReactionEmoji(Box::new(types::ReactionEmoji { emoticon: "👍".into() }));
        let react = stories::SendReaction { add_to_recent: true, peer: me.clone(), story_id: 7, reaction: thumb };
        assert_eq!(rx.try_recv().unwrap(), react.to_bytes());

        let _ = story.unreact().await;
        let empty = base::Reaction::ReactionEmpty;
        let unreact = stories::SendReaction { add_to_recent: true, peer: me.clone(), story_id: 7, reaction: empty };
        assert_eq!(rx.try_recv().unwrap(), unreact.to_bytes());

        let _ = story.forward(0_i64).await;
        assert_eq!(rx.try_recv().unwrap()[..4], messages::SendMedia::CONSTRUCTOR_ID.to_le_bytes());

        let _ = story.read().await;
        assert_eq!(rx.try_recv().unwrap(), stories::ReadStories { peer: me.clone(), max_id: 7 }.to_bytes());

        let _ = story.view().await;
        assert_eq!(rx.try_recv().unwrap(), stories::IncrementStoryViews { peer: me, id: vec![7] }.to_bytes());
    }
}
