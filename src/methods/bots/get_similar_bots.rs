use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{PeerRef, User};

impl Client {
    pub async fn get_similar_bots(&self, bot_id: impl Into<PeerRef>) -> Result<Vec<User>> {
        let request = functions::bots::GetBotRecommendations { bot: self.resolve_user(&bot_id.into()).await? };
        let users = match self.invoke(&request).await? {
            base::users::Users::Users(u) => u.users,
            base::users::Users::UsersSlice(u) => u.users,
        };
        Ok(User::parse_all(self, users))
    }
}
