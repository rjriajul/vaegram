use super::send_options::caption_setters;
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{PeerRef, ReplyParameters, Text};

#[must_use = "a request does nothing until it is awaited"]
pub struct SaveDraft<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    text: Text,
    parse_mode: Option<ParseMode>,
    reply_to: Option<ReplyParameters>,
}

impl Client {
    /// Empty text clears the draft.
    pub fn save_draft(&self, chat_id: impl Into<PeerRef>, text: impl Into<Text>) -> SaveDraft<'_> {
        SaveDraft { client: self, chat_id: chat_id.into(), text: text.into(), parse_mode: None, reply_to: None }
    }
}

impl SaveDraft<'_> {
    pub fn reply_to(mut self, reply: impl Into<ReplyParameters>) -> Self {
        self.reply_to = Some(reply.into());
        self
    }

    async fn send(self) -> Result<()> {
        let client = self.client;
        let (chat_id, peer) = client.resolve(&self.chat_id).await?;
        let (message, entities) = client.write_text(self.text, self.parse_mode).await?;
        let reply_to = match &self.reply_to {
            Some(reply) => {
                let reply = reply.write(client, chat_id).await?;
                Some(base::InputReplyTo::InputReplyToMessage(Box::new(reply)))
            }
            None => None,
        };
        let request = functions::messages::SaveDraft {
            no_webpage: false,
            invert_media: false,
            reply_to,
            peer,
            message,
            entities: (!entities.is_empty()).then_some(entities),
            media: None,
            effect: None,
            suggested_post: None,
            rich_message: None,
        };
        client.invoke(&request).await?;
        Ok(())
    }
}

caption_setters!(SaveDraft);
into_future!(SaveDraft, ());
