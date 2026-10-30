mod delete_stories;
mod export_story_link;
mod get_all_stories;
mod get_chat_stories;
mod get_stories;
mod get_story_viewers;
mod hide_chat_stories;
mod read_stories;
mod send_story_reaction;
mod show_chat_stories;
mod view_stories;

pub use get_story_viewers::GetStoryViewers;

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::enums::MessageEntityType;
    use crate::errors::Error;
    use crate::raw::functions::stories;
    use crate::raw::{Serializable, base, types};
    use crate::types::{Media, ReactionType, Story, StoryViewer};
    use crate::utils::get_peer;

    #[tokio::test]
    async fn reaction_none_sends_empty_reaction() {
        let (client, mut rx) = Client::recording();
        let _ = client.send_story_reaction(0_i64, 7, None).await;
        let expected = stories::SendReaction {
            add_to_recent: true,
            peer: base::InputPeer::InputPeerSelf,
            story_id: 7,
            reaction: base::Reaction::ReactionEmpty,
        };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());

        let _ = client.hide_chat_stories(0_i64).await;
        let expected = stories::TogglePeerStoriesHidden { peer: base::InputPeer::InputPeerSelf, hidden: true };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn empty_story_id_lists_are_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        let results = (
            matches!(client.get_stories(0_i64, &[]).await, Err(Error::InvalidArgument(_))),
            matches!(client.view_stories(0_i64, &[]).await, Err(Error::InvalidArgument(_))),
            matches!(client.delete_stories(0_i64, &[]).await, Err(Error::InvalidArgument(_))),
        );
        assert_eq!(results, (true, true, true));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn parses_stories_and_drops_deleted_ones() {
        let client = Client::offline();
        let views = types::StoryViews {
            has_viewers: true,
            views_count: 12,
            forwards_count: None,
            reactions: None,
            reactions_count: Some(3),
            recent_viewers: None,
        };
        let item = types::StoryItem {
            pinned: true,
            public: true,
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
            entities: None,
            media: base::MessageMedia::MessageMediaEmpty,
            media_areas: None,
            privacy: None,
            views: Some(base::StoryViews::StoryViews(Box::new(views))),
            sent_reaction: None,
            albums: None,
            music: None,
        };
        let skipped = types::StoryItemSkipped { close_friends: true, live: false, id: 6, date: 101, expire_date: 201 };
        let items = vec![
            base::StoryItem::StoryItem(Box::new(item)),
            base::StoryItem::StoryItemDeleted(types::StoryItemDeleted { id: 4 }),
            base::StoryItem::StoryItemSkipped(Box::new(skipped)),
        ];
        let stories = Story::parse_list(&client, &get_peer(42), items, Vec::new(), Vec::new());
        assert_eq!(stories.len(), 2);
        let (full, skipped) = (&stories[0], &stories[1]);
        assert_eq!((full.chat.id, full.id, full.caption.as_deref()), (42, 5, Some("hi")));
        assert_eq!((full.view_count, full.reaction_count, full.is_pinned), (Some(12), Some(3), true));
        assert_eq!((skipped.id, skipped.date, skipped.view_count, skipped.is_close_friends), (6, 101, None, true));
    }

    #[tokio::test]
    async fn custom_emoji_reactions_are_sent() {
        let (client, mut rx) = Client::recording();
        let _ = client.send_story_reaction(0_i64, 7, Some(ReactionType::CustomEmoji(5))).await;
        let emoji = types::ReactionCustomEmoji { document_id: 5 };
        let expected = stories::SendReaction {
            add_to_recent: true,
            peer: base::InputPeer::InputPeerSelf,
            story_id: 7,
            reaction: base::Reaction::ReactionCustomEmoji(emoji),
        };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }

    #[test]
    fn viewers_keep_custom_emoji_and_paid_reactions() {
        let entities = Entities::default();
        let view = |reaction| {
            let view =
                types::StoryView { blocked: false, blocked_my_stories_from: false, user_id: 9, date: 50, reaction };
            StoryViewer::parse(base::StoryView::StoryView(Box::new(view)), &entities).unwrap().reaction
        };
        let custom = base::Reaction::ReactionCustomEmoji(types::ReactionCustomEmoji { document_id: 5 });
        assert_eq!(view(Some(custom)), Some(ReactionType::CustomEmoji(5)));
        assert_eq!(view(Some(base::Reaction::ReactionPaid)), Some(ReactionType::Paid));
        assert_eq!(view(Some(base::Reaction::ReactionEmpty)), None);
    }

    #[test]
    fn stories_keep_caption_entities_and_media() {
        let document = types::Document {
            id: 1,
            access_hash: 2,
            file_reference: vec![3],
            date: 4,
            mime_type: "application/pdf".into(),
            size: 5,
            thumbs: None,
            video_thumbs: None,
            dc_id: 2,
            attributes: Vec::new(),
        };
        let media = types::MessageMediaDocument {
            nopremium: false,
            spoiler: false,
            video: false,
            round: false,
            voice: false,
            document: Some(base::Document::Document(Box::new(document))),
            alt_documents: None,
            video_cover: None,
            video_timestamp: None,
            ttl_seconds: None,
        };
        let bold = base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 2 });
        let item = types::StoryItem {
            pinned: false,
            public: false,
            close_friends: false,
            min: false,
            noforwards: false,
            edited: false,
            contacts: false,
            selected_contacts: false,
            out: false,
            id: 5,
            date: 100,
            from_id: None,
            fwd_from: None,
            expire_date: 200,
            caption: Some("hi".into()),
            entities: Some(vec![bold]),
            media: base::MessageMedia::MessageMediaDocument(Box::new(media)),
            media_areas: None,
            privacy: None,
            views: None,
            sent_reaction: None,
            albums: None,
            music: None,
        };
        let story =
            Story::parse(&get_peer(42), base::StoryItem::StoryItem(Box::new(item)), &Entities::default()).unwrap();
        let entities = story.caption_entities.unwrap();
        assert_eq!((entities.len(), entities[0].kind, entities[0].length), (1, MessageEntityType::Bold, 2));
        assert!(story.media.as_ref().and_then(Media::file_location).is_some());
    }

    #[test]
    fn viewers_skip_reposts_and_keep_emoji() {
        let entities = Entities::default();
        let fire = base::Reaction::ReactionEmoji(Box::new(types::ReactionEmoji { emoticon: "🔥".into() }));
        let view = types::StoryView {
            blocked: false,
            blocked_my_stories_from: true,
            user_id: 9,
            date: 50,
            reaction: Some(fire),
        };
        let viewer = StoryViewer::parse(base::StoryView::StoryView(Box::new(view)), &entities).unwrap();
        assert_eq!(
            (viewer.user.id, viewer.date, viewer.reaction, viewer.is_blocked),
            (9, 50, Some(ReactionType::Emoji("🔥".into())), true)
        );

        let repost = types::StoryViewPublicRepost {
            blocked: false,
            blocked_my_stories_from: false,
            peer_id: get_peer(9),
            story: base::StoryItem::StoryItemDeleted(types::StoryItemDeleted { id: 1 }),
        };
        assert!(StoryViewer::parse(base::StoryView::StoryViewPublicRepost(Box::new(repost)), &entities).is_none());
    }
}
