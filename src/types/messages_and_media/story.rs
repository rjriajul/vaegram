use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::StoriesPrivacyRules;
use crate::errors::{Error, Result};
use crate::methods::{DownloadMedia, DownloadSource};
use crate::raw::base;
use crate::types::{Chat, Media, MessageEntity, PrivacyRule, ReactionCount, ReactionType, StoryArea, User};

/// A story posted to a chat's story feed.
#[derive(Debug, Clone)]
pub struct Story {
    /// The chat whose story feed holds the story.
    pub chat: Chat,
    /// Id of the story within `chat`.
    pub id: i32,
    /// Unix time when the story was posted.
    pub date: i32,
    /// Unix time when the story expires from the feed, unless it's pinned.
    pub expire_date: i32,
    /// The user who posted the story. `None` if a channel or group posted it.
    pub from_user: Option<User>,
    /// The channel or group that posted the story. `None` if a user posted it.
    pub sender_chat: Option<Chat>,
    /// The user who posted the story that this one reposts. `None` if it isn't a repost, or the
    /// original author is a chat or hides their account.
    pub forward_from: Option<User>,
    /// The channel or group that posted the story that this one reposts. `None` if it isn't a repost of a
    /// chat's story.
    pub forward_from_chat: Option<Chat>,
    /// Name of the original author when they hide their account.
    pub forward_sender_name: Option<String>,
    /// Id of the reposted story in its original chat.
    pub forward_from_story_id: Option<i32>,
    /// `true` if the reposter edited the story after reposting it.
    pub is_forward_modified: bool,
    /// Caption text. `None` if the story has no caption.
    pub caption: Option<String>,
    /// Formatting entities in `caption`.
    pub caption_entities: Option<Vec<MessageEntity>>,
    /// The story's photo or video. `None` if Telegram sent no media for it.
    pub media: Option<Media>,
    /// Interactive areas placed on the media, such as locations and links.
    pub media_areas: Vec<StoryArea>,
    /// Number of views. `None` when Telegram doesn't send view counters.
    pub view_count: Option<i32>,
    /// Total number of reactions. `None` when Telegram doesn't send it.
    pub reaction_count: Option<i32>,
    /// Number of times the story was forwarded or reposted. `None` when Telegram doesn't send it.
    pub forward_count: Option<i32>,
    /// Reactions on the story with their counts.
    pub reactions: Vec<ReactionCount>,
    /// Who can see the story. `None` unless the account posted the story and its rules match one of the
    /// [`StoriesPrivacyRules`] variants.
    pub privacy: Option<StoriesPrivacyRules>,
    /// `true` if the story is pinned to the profile and stays after it expires.
    pub is_pinned: bool,
    /// `true` if the story is visible to everyone.
    pub is_public: bool,
    /// `true` if only close friends can see the story.
    pub is_close_friends: bool,
    /// `true` if only contacts can see the story.
    pub is_contacts: bool,
    /// `true` if only selected contacts can see the story.
    pub is_selected_contacts: bool,
    /// `true` if the story was edited after posting.
    pub is_edited: bool,
    /// `true` if the current account posted the story.
    pub outgoing: bool,
    /// `true` if the story can't be forwarded or saved.
    pub has_protected_content: bool,
    /// `true` for a placeholder Telegram sends instead of the full story. Only `id`, the dates and
    /// `is_close_friends` are set.
    pub is_skipped: bool,
    client: Client,
}

impl Story {
    pub(crate) fn parse(
        client: &Client,
        peer: &base::Peer,
        item: base::StoryItem,
        entities: &Entities,
    ) -> Option<Story> {
        let mut story = Story::from_media(client, peer, 0, entities);
        match item {
            base::StoryItem::StoryItemDeleted(_) => return None,
            base::StoryItem::StoryItemSkipped(s) => {
                (story.id, story.date, story.expire_date) = (s.id, s.date, s.expire_date);
                story.is_close_friends = s.close_friends;
                story.is_skipped = true;
            }
            base::StoryItem::StoryItem(s) => {
                let views = s.views.map(|base::StoryViews::StoryViews(v)| *v);
                (story.id, story.date, story.expire_date, story.caption) = (s.id, s.date, s.expire_date, s.caption);
                story.caption_entities = MessageEntity::parse_all(s.entities.as_deref().unwrap_or_default(), entities);
                story.media = Media::parse(client, &s.media, entities);
                story.media_areas = StoryArea::parse_all(s.media_areas.as_deref().unwrap_or_default(), entities);
                let rules: Vec<PrivacyRule> =
                    s.privacy.into_iter().flatten().map(|rule| PrivacyRule::parse(rule, entities)).collect();
                story.privacy = StoriesPrivacyRules::from_rules(&rules);
                story.view_count = views.as_ref().map(|v| v.views_count);
                story.forward_count = views.as_ref().and_then(|v| v.forwards_count);
                story.reactions = views
                    .as_ref()
                    .and_then(|v| v.reactions.as_deref())
                    .map(|r| r.iter().filter_map(ReactionCount::parse).collect())
                    .unwrap_or_default();
                story.reaction_count = views.and_then(|v| v.reactions_count);
                (story.is_pinned, story.is_public, story.is_close_friends, story.is_edited) =
                    (s.pinned, s.public, s.close_friends, s.edited);
                (story.is_contacts, story.is_selected_contacts) = (s.contacts, s.selected_contacts);
                (story.outgoing, story.has_protected_content) = (s.out, s.noforwards);
                // The author is the explicit one when Telegram names it, else whoever owns the story's feed.
                match s.from_id.as_ref().unwrap_or(peer) {
                    base::Peer::PeerUser(u) => story.from_user = Some(User::parse(u.user_id, entities)),
                    other => story.sender_chat = Some(Chat::parse(other, entities, false)),
                }
                if let Some(base::StoryFwdHeader::StoryFwdHeader(f)) = s.fwd_from {
                    match &f.from {
                        Some(base::Peer::PeerUser(u)) => story.forward_from = Some(User::parse(u.user_id, entities)),
                        Some(other) => story.forward_from_chat = Some(Chat::parse(other, entities, false)),
                        None => {}
                    }
                    story.forward_sender_name = f.from_name;
                    story.forward_from_story_id = f.story_id;
                    story.is_forward_modified = f.modified;
                }
            }
        }
        Some(story)
    }

    /// A story a message points at without carrying it, or one deleted since: only `chat` and `id` are known, so
    /// the dates are 0, the counters, authors and media `None`, the lists empty and the flags `false`.
    pub(crate) fn from_media(client: &Client, peer: &base::Peer, id: i32, entities: &Entities) -> Story {
        Story {
            chat: Chat::parse(peer, entities, false),
            id,
            date: 0,
            expire_date: 0,
            from_user: None,
            sender_chat: None,
            forward_from: None,
            forward_from_chat: None,
            forward_sender_name: None,
            forward_from_story_id: None,
            is_forward_modified: false,
            caption: None,
            caption_entities: None,
            media: None,
            media_areas: Vec::new(),
            view_count: None,
            reaction_count: None,
            forward_count: None,
            reactions: Vec::new(),
            privacy: None,
            is_pinned: false,
            is_public: false,
            is_close_friends: false,
            is_contacts: false,
            is_selected_contacts: false,
            is_edited: false,
            outgoing: false,
            has_protected_content: false,
            is_skipped: false,
            client: client.clone(),
        }
    }

    /// Returns the client this story came from.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Downloads the story's photo or video.
    ///
    /// Awaiting it fails with [`Error::InvalidArgument`] if the story has no downloadable media.
    pub fn download<'a>(&self, client: &'a Client) -> DownloadMedia<'a> {
        let location = self.media.as_ref().and_then(Media::file_location).cloned();
        let location = location.ok_or(Error::InvalidArgument("this story has no photo or document to download"));
        DownloadMedia::new(client, location, Some(DownloadSource::Story(self.chat.id, self.id)))
    }

    pub(crate) fn parse_list(
        client: &Client,
        peer: &base::Peer,
        items: Vec<base::StoryItem>,
        users: Vec<base::User>,
        chats: Vec<base::Chat>,
    ) -> Vec<Story> {
        let entities = Entities::new(client, users, chats);
        items.into_iter().filter_map(|item| Story::parse(client, peer, item, &entities)).collect()
    }

    pub(crate) fn from_updates(client: &Client, updates: base::Updates) -> Result<Story> {
        let (updates, users, chats) = match updates {
            base::Updates::Updates(u) => (u.updates, u.users, u.chats),
            base::Updates::UpdatesCombined(u) => (u.updates, u.users, u.chats),
            base::Updates::UpdateShort(u) => (vec![u.update], Vec::new(), Vec::new()),
            _ => Default::default(),
        };
        let entities = Entities::new(client, users, chats);
        updates
            .into_iter()
            .find_map(|u| match u {
                base::Update::UpdateStory(u) => {
                    let u = *u;
                    Story::parse(client, &u.peer, u.story, &entities)
                }
                _ => None,
            })
            .ok_or(Error::Unexpected("Telegram sent no story back"))
    }
}

/// A user who viewed a story.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryViewer {
    /// The user who viewed the story.
    pub user: User,
    /// Unix time when the viewer opened the story.
    pub date: i32,
    /// Reaction the viewer left. `None` if they didn't react.
    pub reaction: Option<ReactionType>,
    /// `true` if the viewer is on the account's block list.
    pub is_blocked: bool,
    /// `true` if the viewer is blocked from seeing the account's stories only.
    pub is_blocked_my_stories_from: bool,
}

impl StoryViewer {
    pub(crate) fn parse(view: base::StoryView, entities: &Entities) -> Option<StoryViewer> {
        let base::StoryView::StoryView(v) = view else { return None };
        Some(StoryViewer {
            user: User::parse(v.user_id, entities),
            date: v.date,
            reaction: v.reaction.as_ref().and_then(ReactionType::parse),
            is_blocked: v.blocked,
            is_blocked_my_stories_from: v.blocked_my_stories_from,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Story;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::enums::StoriesPrivacyRules;
    use crate::raw::{base, types};
    use crate::types::{ReactionType, StoryAreaType};
    use crate::utils::{get_channel_id, get_peer};

    fn item() -> types::StoryItem {
        types::StoryItem {
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
            caption: None,
            entities: None,
            media: base::MessageMedia::MessageMediaEmpty,
            media_areas: None,
            privacy: None,
            views: None,
            sent_reaction: None,
            albums: None,
            music: None,
        }
    }

    fn parse(peer: &base::Peer, item: types::StoryItem) -> Story {
        let item = base::StoryItem::StoryItem(Box::new(item));
        Story::parse(&Client::offline(), peer, item, &Entities::default()).unwrap()
    }

    #[test]
    fn reads_author_forward_and_flags() {
        let own = parse(&get_peer(42), item());
        assert_eq!(own.from_user.as_ref().map(|u| u.id), Some(42));
        assert!(own.sender_chat.is_none() && own.forward_from.is_none() && own.forward_sender_name.is_none());

        let channel = get_peer(get_channel_id(9));
        let posted = parse(&channel, item());
        assert_eq!(posted.sender_chat.as_ref().map(|c| c.id), Some(get_channel_id(9)));
        assert!(posted.from_user.is_none());

        // An explicit author wins over the feed's owner.
        let by_user = parse(&channel, types::StoryItem { from_id: Some(get_peer(8)), ..item() });
        assert_eq!((by_user.from_user.map(|u| u.id), by_user.sender_chat.is_none()), (Some(8), true));

        let header = |from, from_name: Option<&str>| {
            let header = types::StoryFwdHeader {
                modified: true,
                from,
                from_name: from_name.map(str::to_owned),
                story_id: Some(9),
            };
            Some(base::StoryFwdHeader::StoryFwdHeader(Box::new(header)))
        };
        let flagged = types::StoryItem {
            noforwards: true,
            out: true,
            contacts: true,
            selected_contacts: true,
            fwd_from: header(Some(get_peer(6)), None),
            ..item()
        };
        let story = parse(&get_peer(42), flagged);
        assert_eq!(story.forward_from.as_ref().map(|u| u.id), Some(6));
        assert_eq!((story.forward_from_story_id, story.is_forward_modified), (Some(9), true));
        assert!(story.has_protected_content && story.outgoing && story.is_contacts && story.is_selected_contacts);

        let hidden = parse(&get_peer(42), types::StoryItem { fwd_from: header(None, Some("Hidden")), ..item() });
        assert_eq!(hidden.forward_sender_name.as_deref(), Some("Hidden"));
        let from_chat = parse(&get_peer(42), types::StoryItem { fwd_from: header(Some(channel), None), ..item() });
        assert_eq!(from_chat.forward_from_chat.map(|c| c.id), Some(get_channel_id(9)));
    }

    #[test]
    fn reads_reactions_and_forward_count() {
        let fire = base::Reaction::ReactionEmoji(Box::new(types::ReactionEmoji { emoticon: "🔥".into() }));
        let count = types::ReactionCount { chosen_order: Some(0), reaction: fire, count: 3 };
        let views = types::StoryViews {
            has_viewers: false,
            views_count: 12,
            forwards_count: Some(4),
            reactions: Some(vec![base::ReactionCount::ReactionCount(Box::new(count))]),
            reactions_count: Some(3),
            recent_viewers: None,
        };
        let views = Some(base::StoryViews::StoryViews(Box::new(views)));
        let story = parse(&get_peer(42), types::StoryItem { views, ..item() });
        assert_eq!((story.forward_count, story.view_count, story.reaction_count), (Some(4), Some(12), Some(3)));
        assert_eq!(story.reactions.len(), 1);
        assert_eq!(story.reactions[0].kind, ReactionType::Emoji("🔥".into()));
        assert_eq!((story.reactions[0].total_count, story.reactions[0].chosen_order), (3, Some(0)));

        let bare = parse(&get_peer(42), item());
        assert!(bare.forward_count.is_none() && bare.reactions.is_empty());
    }

    #[test]
    fn own_story_privacy_round_trips() {
        let allow = base::PrivacyRule::PrivacyValueAllowAll;
        let except = types::PrivacyValueDisallowUsers { users: vec![3] };
        let privacy = Some(vec![allow, base::PrivacyRule::PrivacyValueDisallowUsers(Box::new(except))]);
        let story = parse(&get_peer(42), types::StoryItem { privacy, ..item() });
        assert_eq!(story.privacy, Some(StoriesPrivacyRules::Public { except: vec![3] }));

        let odd = Some(vec![base::PrivacyRule::PrivacyValueDisallowAll]);
        assert_eq!(parse(&get_peer(42), types::StoryItem { privacy: odd, ..item() }).privacy, None);
        assert_eq!(parse(&get_peer(42), item()).privacy, None);
    }

    #[test]
    fn skipped_story_is_marked() {
        let skipped = types::StoryItemSkipped { close_friends: true, live: false, id: 6, date: 101, expire_date: 201 };
        let item = base::StoryItem::StoryItemSkipped(Box::new(skipped));
        let story = Story::parse(&Client::offline(), &get_peer(42), item, &Entities::default()).unwrap();
        assert!(story.is_skipped && story.is_close_friends);
        assert!(story.from_user.is_none() && story.media_areas.is_empty());
        assert!(!parse(&get_peer(42), item_with_area()).is_skipped);
    }

    fn item_with_area() -> types::StoryItem {
        let coordinates = base::MediaAreaCoordinates::MediaAreaCoordinates(Box::new(types::MediaAreaCoordinates {
            x: 1.0,
            y: 2.0,
            w: 3.0,
            h: 4.0,
            rotation: 0.0,
            radius: None,
        }));
        let url = types::MediaAreaUrl { coordinates, url: "https://example.com".into() };
        types::StoryItem { media_areas: Some(vec![base::MediaArea::MediaAreaUrl(Box::new(url))]), ..item() }
    }

    #[test]
    fn media_areas_are_read() {
        let story = parse(&get_peer(42), item_with_area());
        assert_eq!(story.media_areas.len(), 1);
        assert_eq!(story.media_areas[0].kind, StoryAreaType::Link { url: "https://example.com".into() });
    }
}
