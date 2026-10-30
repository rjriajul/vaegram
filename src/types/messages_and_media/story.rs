use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::methods::{DownloadMedia, DownloadSource};
use crate::raw::base;
use crate::types::{Chat, Media, MessageEntity, ReactionType, User};

#[derive(Debug, Clone)]
pub struct Story {
    pub chat: Chat,
    pub id: i32,
    pub date: i32,
    pub expire_date: i32,
    pub caption: Option<String>,
    pub caption_entities: Option<Vec<MessageEntity>>,
    pub media: Option<Media>,
    pub view_count: Option<i32>,
    pub reaction_count: Option<i32>,
    pub is_pinned: bool,
    pub is_public: bool,
    pub is_close_friends: bool,
    pub is_edited: bool,
}

impl Story {
    pub(crate) fn parse(peer: &base::Peer, item: base::StoryItem, entities: &Entities) -> Option<Story> {
        let chat = Chat::parse(peer, entities, false);
        let mut story = Story {
            chat,
            id: 0,
            date: 0,
            expire_date: 0,
            caption: None,
            caption_entities: None,
            media: None,
            view_count: None,
            reaction_count: None,
            is_pinned: false,
            is_public: false,
            is_close_friends: false,
            is_edited: false,
        };
        match item {
            base::StoryItem::StoryItemDeleted(_) => return None,
            base::StoryItem::StoryItemSkipped(s) => {
                (story.id, story.date, story.expire_date) = (s.id, s.date, s.expire_date);
                story.is_close_friends = s.close_friends;
            }
            base::StoryItem::StoryItem(s) => {
                let views = s.views.map(|base::StoryViews::StoryViews(v)| *v);
                (story.id, story.date, story.expire_date, story.caption) = (s.id, s.date, s.expire_date, s.caption);
                story.caption_entities = MessageEntity::parse_all(s.entities.as_deref().unwrap_or_default(), entities);
                story.media = Media::parse(&s.media, entities);
                story.view_count = views.as_ref().map(|v| v.views_count);
                story.reaction_count = views.and_then(|v| v.reactions_count);
                (story.is_pinned, story.is_public, story.is_close_friends, story.is_edited) =
                    (s.pinned, s.public, s.close_friends, s.edited);
            }
        }
        Some(story)
    }

    /// Fails at `.await` for a story without a photo or document.
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
        items.into_iter().filter_map(|item| Story::parse(peer, item, &entities)).collect()
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
                    Story::parse(&u.peer, u.story, &entities)
                }
                _ => None,
            })
            .ok_or(Error::Unexpected("Telegram sent no story back"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryViewer {
    pub user: User,
    pub date: i32,
    pub reaction: Option<ReactionType>,
    pub is_blocked: bool,
}

impl StoryViewer {
    pub(crate) fn parse(view: base::StoryView, entities: &Entities) -> Option<StoryViewer> {
        let base::StoryView::StoryView(v) = view else { return None };
        Some(StoryViewer {
            user: User::parse(v.user_id, entities),
            date: v.date,
            reaction: v.reaction.as_ref().and_then(ReactionType::parse),
            is_blocked: v.blocked || v.blocked_my_stories_from,
        })
    }
}
