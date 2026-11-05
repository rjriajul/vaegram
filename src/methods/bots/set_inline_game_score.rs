use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InlineMessageId, PeerRef};

/// Request built by [`Client::set_inline_game_score`]. Awaiting it returns whether Telegram accepted the score.
#[must_use = "a request does nothing until it is awaited"]
pub struct SetInlineGameScore<'a> {
    client: &'a Client,
    inline_message_id: InlineMessageId,
    user_id: PeerRef,
    score: i32,
    force: bool,
    edit_message: bool,
}

impl Client {
    /// Sets a user's score in a game sent through inline mode.
    /// Fails unless the score beats the user's current one. See [`SetInlineGameScore::force`].
    pub fn set_inline_game_score(
        &self,
        inline_message_id: InlineMessageId,
        user_id: impl Into<PeerRef>,
        score: i32,
    ) -> SetInlineGameScore<'_> {
        SetInlineGameScore {
            client: self,
            inline_message_id,
            user_id: user_id.into(),
            score,
            force: false,
            edit_message: true,
        }
    }
}

impl SetInlineGameScore<'_> {
    /// Sets the score even if it doesn't beat the user's current one, so it can go down.
    pub fn force(mut self) -> Self {
        self.force = true;
        self
    }

    /// Keeps the game message as it is instead of showing the new scoreboard.
    pub fn disable_edit_message(mut self) -> Self {
        self.edit_message = false;
        self
    }

    async fn send(self) -> Result<bool> {
        let client = self.client;
        let dc_id = self.inline_message_id.dc_id();
        let request = functions::messages::SetInlineGameScore {
            edit_message: self.edit_message,
            force: self.force,
            id: self.inline_message_id.0,
            user_id: client.resolve_user(&self.user_id).await?,
            score: self.score,
        };
        client.invoke_inline(dc_id, request).await
    }
}

into_future!(SetInlineGameScore, bool);
