use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::{Chat, MessageEntity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderInviteLinkInfo {
    pub folder_id: Option<i32>,
    /// `None` once joined: Telegram doesn't send the title of a list the account already has.
    pub title: Option<String>,
    pub title_entities: Vec<MessageEntity>,
    pub emoji: Option<String>,
    pub missing_chats: Vec<Chat>,
    pub added_chats: Vec<Chat>,
}

impl FolderInviteLinkInfo {
    pub(crate) fn parse(client: &Client, invite: base::chatlists::ChatlistInvite) -> FolderInviteLinkInfo {
        use base::chatlists::ChatlistInvite as I;
        match invite {
            I::ChatlistInvite(i) => {
                let base::TextWithEntities::TextWithEntities(title) = i.title;
                let entities = Entities::new(client, i.users, i.chats);
                let missing_chats = chats(&i.peers, &entities);
                FolderInviteLinkInfo {
                    folder_id: None,
                    title_entities: MessageEntity::parse_all(&title.entities, &entities).unwrap_or_default(),
                    title: Some(title.text),
                    emoji: i.emoticon,
                    missing_chats,
                    added_chats: Vec::new(),
                }
            }
            I::ChatlistInviteAlready(i) => {
                let entities = Entities::new(client, i.users, i.chats);
                FolderInviteLinkInfo {
                    folder_id: Some(i.filter_id),
                    title: None,
                    title_entities: Vec::new(),
                    emoji: None,
                    missing_chats: chats(&i.missing_peers, &entities),
                    added_chats: chats(&i.already_peers, &entities),
                }
            }
        }
    }
}

fn chats(peers: &[base::Peer], entities: &Entities) -> Vec<Chat> {
    peers.iter().map(|p| Chat::parse(p, entities, false)).collect()
}
