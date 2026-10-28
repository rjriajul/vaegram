use crate::Client;
use crate::errors::Result;
use crate::methods::messages::{SendOptions, send_builder};
use crate::raw::{base, types};
use crate::types::{Message, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendGame<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    game_short_name: String,
    options: SendOptions,
}

impl Client {
    /// Without a reply markup Telegram adds a Play button.
    pub fn send_game(&self, chat_id: impl Into<PeerRef>, game_short_name: impl Into<String>) -> SendGame<'_> {
        SendGame {
            client: self,
            chat_id: chat_id.into(),
            game_short_name: game_short_name.into(),
            options: SendOptions::default(),
        }
    }
}

impl SendGame<'_> {
    async fn send(self) -> Result<Message> {
        let game =
            types::InputGameShortName { bot_id: base::InputUser::InputUserSelf, short_name: self.game_short_name };
        let media = types::InputMediaGame { id: base::InputGame::InputGameShortName(Box::new(game)) };
        let media = base::InputMedia::InputMediaGame(Box::new(media));
        self.client.send_media(self.chat_id, media, "", None, self.options).await
    }
}

send_builder!(SendGame);
