use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{CallbackAnswer, PeerRef};
use crate::utils::compute_password_check;

#[must_use = "a request does nothing until it is awaited"]
pub struct RequestCallbackAnswer<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    data: Option<Vec<u8>>,
    password: Option<String>,
}

impl Client {
    pub fn request_callback_answer(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        data: impl Into<Vec<u8>>,
    ) -> RequestCallbackAnswer<'_> {
        RequestCallbackAnswer {
            client: self,
            chat_id: chat_id.into(),
            message_id,
            data: Some(data.into()),
            password: None,
        }
    }
}

impl RequestCallbackAnswer<'_> {
    /// Presses the "play" button of a game message instead; `data` is ignored.
    pub fn game(mut self) -> Self {
        self.data = None;
        self
    }

    /// Your 2FA password, for buttons that ask for it (like transferring a bot).
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    async fn send(self) -> Result<CallbackAnswer> {
        let client = self.client;
        let password = match &self.password {
            Some(password) => {
                let base::account::Password::Password(p) = client.invoke(&functions::account::GetPassword {}).await?;
                Some(compute_password_check(&p, password)?)
            }
            None => None,
        };
        let request = functions::messages::GetBotCallbackAnswer {
            game: self.data.is_none(),
            peer: client.resolve_peer(self.chat_id).await?,
            msg_id: self.message_id,
            data: self.data,
            password,
        };
        Ok(CallbackAnswer::parse(client.invoke(&request).await?))
    }
}

into_future!(RequestCallbackAnswer, CallbackAnswer);
