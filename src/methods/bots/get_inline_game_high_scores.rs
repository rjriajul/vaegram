use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{GameHighScore, InlineMessageId, PeerRef};

impl Client {
    /// Returns the top scores of an inline-mode game, plus `user_id` and their neighbours.
    pub async fn get_inline_game_high_scores(
        &self,
        inline_message_id: InlineMessageId,
        user_id: impl Into<PeerRef>,
    ) -> Result<Vec<GameHighScore>> {
        let dc_id = inline_message_id.dc_id();
        let request = functions::messages::GetInlineGameHighScores {
            id: inline_message_id.0,
            user_id: self.resolve_user(&user_id.into()).await?,
        };
        Ok(self.high_scores(self.invoke_inline(dc_id, request).await?))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::InlineMessageId;

    #[tokio::test]
    async fn the_id_is_sent_as_given() {
        let (client, mut rx) = Client::recording();
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        let raw = types::InputBotInlineMessageID64 { dc_id: 2, owner_id: 5, id: 6, access_hash: 7 };
        let raw = base::InputBotInlineMessageID::InputBotInlineMessageID64(Box::new(raw));
        let _ = client.get_inline_game_high_scores(InlineMessageId(raw.clone()), 7).await;
        let user_id = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 7, access_hash: 1 }));
        let expected = functions::messages::GetInlineGameHighScores { id: raw, user_id };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }
}
