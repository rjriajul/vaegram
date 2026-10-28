use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{ChatBoost, PeerRef};

impl Client {
    /// The boosts a user added to a chat; the bot must be an administrator there.
    pub async fn get_user_chat_boosts(
        &self,
        chat_id: impl Into<PeerRef>,
        user_id: impl Into<PeerRef>,
    ) -> Result<Vec<ChatBoost>> {
        let peer = self.resolve_peer(chat_id).await?;
        let user_id = self.resolve_user(&user_id.into()).await?;
        let base::premium::BoostsList::BoostsList(list) =
            self.invoke(&functions::premium::GetUserBoosts { peer, user_id }).await?;
        let list = *list;
        let entities = Entities::new(self, list.users, Vec::new());
        Ok(list.boosts.into_iter().filter_map(|b| ChatBoost::parse(b, &entities)).collect())
    }
}
