use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{Chat, Draft};

impl Client {
    /// Returns every draft of the account, each with its chat. Drafts in direct messages chats are left out.
    pub async fn get_all_drafts(&self) -> Result<Vec<Draft>> {
        let (updates, users, chats) = match self.invoke(&functions::messages::GetAllDrafts {}).await? {
            base::Updates::Updates(u) => (u.updates, u.users, u.chats),
            base::Updates::UpdatesCombined(u) => (u.updates, u.users, u.chats),
            base::Updates::UpdateShort(u) => (vec![u.update], Vec::new(), Vec::new()),
            _ => Default::default(),
        };
        let entities = Entities::new(self, users, chats);
        Ok(updates
            .iter()
            .filter_map(|u| match u {
                // Drafts in a channel's direct messages carry saved_peer_id; those aren't regular chats.
                base::Update::UpdateDraftMessage(u) if u.saved_peer_id.is_none() => {
                    Draft::parse(&u.draft, Chat::parse(&u.peer, &entities, false), u.top_msg_id, &entities)
                }
                _ => None,
            })
            .collect())
    }
}
