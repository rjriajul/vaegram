use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{GameHighScore, InlineMessageId, PeerRef};

impl Client {
    pub async fn get_inline_game_high_scores(
        &self,
        inline_message_id: &InlineMessageId,
        user_id: impl Into<PeerRef>,
    ) -> Result<Vec<GameHighScore>> {
        let request = functions::messages::GetInlineGameHighScores {
            id: inline_message_id.0.clone(),
            user_id: self.resolve_user(&user_id.into()).await?,
        };
        Ok(self.high_scores(self.invoke_inline(inline_message_id.dc_id(), request).await?))
    }
}
