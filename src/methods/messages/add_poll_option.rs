use super::send_poll::text_with_entities;
use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{InputPollOption, PeerRef};

impl Client {
    pub async fn add_poll_option<'a>(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        option: impl Into<InputPollOption<'a>>,
    ) -> Result<()> {
        let InputPollOption { text, media } = option.into();
        let peer = self.resolve_peer(chat_id).await?;
        let media = match media {
            Some(media) => Some(media.upload(self, &peer, None).await?),
            None => None,
        };
        let answer = types::InputPollAnswer { text: text_with_entities(text), media };
        let answer = base::PollAnswer::InputPollAnswer(Box::new(answer));
        self.invoke(&functions::messages::AddPollAnswer { peer, msg_id: message_id, answer }).await.map(drop)
    }
}
