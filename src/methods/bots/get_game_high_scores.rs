use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{GameHighScore, PeerRef};

impl Client {
    /// The top scores of the game in a chat message, plus `user_id` and their neighbours if they're further down.
    pub async fn get_game_high_scores(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        user_id: impl Into<PeerRef>,
    ) -> Result<Vec<GameHighScore>> {
        let request = functions::messages::GetGameHighScores {
            peer: self.resolve(&chat_id.into()).await?.1,
            id: message_id,
            user_id: self.resolve_user(&user_id.into()).await?,
        };
        Ok(self.high_scores(self.invoke(&request).await?))
    }

    pub(crate) fn high_scores(&self, scores: base::messages::HighScores) -> Vec<GameHighScore> {
        let base::messages::HighScores::HighScores(raw) = scores;
        let types::messages::HighScores { scores, users } = *raw;
        let entities = Entities::new(self, users, Vec::new());
        scores.iter().map(|s| GameHighScore::parse(s, &entities)).collect()
    }
}
