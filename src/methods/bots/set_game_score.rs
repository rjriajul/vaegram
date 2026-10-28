use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Message, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SetGameScore<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    user_id: PeerRef,
    score: i32,
    force: bool,
    edit_message: bool,
}

impl Client {
    /// Fails unless the score beats the user's current one; see [`SetGameScore::force`].
    pub fn set_game_score(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        user_id: impl Into<PeerRef>,
        score: i32,
    ) -> SetGameScore<'_> {
        SetGameScore {
            client: self,
            chat_id: chat_id.into(),
            message_id,
            user_id: user_id.into(),
            score,
            force: false,
            edit_message: true,
        }
    }
}

impl SetGameScore<'_> {
    pub fn force(mut self) -> Self {
        self.force = true;
        self
    }

    /// Keeps the game message as it is instead of showing the new scoreboard; the result is then `None`.
    pub fn disable_edit_message(mut self) -> Self {
        self.edit_message = false;
        self
    }

    async fn send(self) -> Result<Option<Message>> {
        let client = self.client;
        let (chat_id, peer) = client.resolve(&self.chat_id).await?;
        let request = functions::messages::SetGameScore {
            edit_message: self.edit_message,
            force: self.force,
            peer,
            id: self.message_id,
            user_id: client.resolve_user(&self.user_id).await?,
            score: self.score,
        };
        let updates = client.invoke(&request).await?;
        if !self.edit_message {
            return Ok(None);
        }
        Message::from_updates(client, updates, chat_id, (String::new(), None)).map(Some)
    }
}

into_future!(SetGameScore, Option<Message>);
