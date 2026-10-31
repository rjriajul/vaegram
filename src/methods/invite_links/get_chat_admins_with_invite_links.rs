use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{ChatAdminWithInviteLinks, PeerRef};

impl Client {
    pub async fn get_chat_admins_with_invite_links(
        &self,
        chat_id: impl Into<PeerRef>,
    ) -> Result<Vec<ChatAdminWithInviteLinks>> {
        let request = functions::messages::GetAdminsWithInvites { peer: self.resolve_peer(chat_id).await? };
        let base::messages::ChatAdminsWithInvites::ChatAdminsWithInvites(found) = self.invoke(&request).await?;
        let found = *found;
        let entities = Entities::new(self, found.users, Vec::new());
        let admins = found.admins.into_iter().map(|base::ChatAdminWithInvites::ChatAdminWithInvites(admin)| {
            ChatAdminWithInviteLinks::parse(*admin, &entities)
        });
        Ok(admins.collect())
    }
}
