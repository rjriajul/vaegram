use crate::dispatcher::Entities;
use crate::raw::base;
use crate::utils::get_channel_id;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Community {
    pub id: i64,
    pub name: String,
}

impl Community {
    // Communities live in the channel id space, so they share the -100 Bot API prefix.
    pub(crate) fn parse(community_id: i64, entities: &Entities) -> Community {
        let id = get_channel_id(community_id);
        let name = match entities.chats.get(&id) {
            Some(base::Chat::Community(c)) => c.title.clone(),
            Some(base::Chat::CommunityForbidden(c)) => c.title.clone(),
            _ => String::new(),
        };
        Community { id, name }
    }

    pub(crate) fn linked(chat_id: i64, entities: &Entities) -> Option<Community> {
        let community_id = match (entities.users.get(&chat_id), entities.chats.get(&chat_id)) {
            (Some(base::User::User(u)), _) => u.linked_community_id,
            (_, Some(base::Chat::Channel(c))) => c.linked_community_id,
            _ => None,
        }?;
        Some(Community::parse(community_id, entities))
    }
}
