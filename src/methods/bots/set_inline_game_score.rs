use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InlineMessageId, PeerRef};

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
    pub fn force(mut self) -> Self {
        self.force = true;
        self
    }

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
