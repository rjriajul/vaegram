use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::base;
use crate::types::Chat;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Story {
    pub chat: Chat,
    pub id: i32,
}

impl Story {
    pub(crate) fn from_updates(client: &Client, updates: base::Updates) -> Result<Story> {
        let (updates, users, chats) = match updates {
            base::Updates::Updates(u) => (u.updates, u.users, u.chats),
            base::Updates::UpdatesCombined(u) => (u.updates, u.users, u.chats),
            base::Updates::UpdateShort(u) => (vec![u.update], Vec::new(), Vec::new()),
            _ => return Err(Error::Unexpected("Telegram sent no story back")),
        };
        let entities = Entities::new(client, users, chats);
        updates
            .iter()
            .find_map(|u| match u {
                base::Update::UpdateStory(u) => {
                    let id = match &u.story {
                        base::StoryItem::StoryItem(s) => s.id,
                        base::StoryItem::StoryItemSkipped(s) => s.id,
                        base::StoryItem::StoryItemDeleted(_) => return None,
                    };
                    Some(Story { chat: Chat::parse(&u.peer, &entities, false), id })
                }
                _ => None,
            })
            .ok_or(Error::Unexpected("Telegram sent no story back"))
    }
}
