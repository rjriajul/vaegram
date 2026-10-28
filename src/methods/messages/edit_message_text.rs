use super::send_options::caption_setters;
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{Message, MessageText, PeerRef, ReplyMarkup};

#[must_use = "a request does nothing until it is awaited"]
pub struct EditMessageText<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    text: MessageText,
    parse_mode: Option<ParseMode>,
    reply_markup: Option<ReplyMarkup>,
    business_connection_id: Option<String>,
}

impl Client {
    pub fn edit_message_text(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        text: impl Into<MessageText>,
    ) -> EditMessageText<'_> {
        EditMessageText {
            client: self,
            chat_id: chat_id.into(),
            message_id,
            text: text.into(),
            parse_mode: None,
            reply_markup: None,
            business_connection_id: None,
        }
    }
}

impl EditMessageText<'_> {
    pub fn reply_markup(mut self, markup: impl Into<ReplyMarkup>) -> Self {
        self.reply_markup = Some(markup.into());
        self
    }

    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.business_connection_id = Some(id.into());
        self
    }

    async fn send(self) -> Result<Message> {
        let client = self.client;
        let (chat_id, peer) = client.resolve(&self.chat_id).await?;
        let (message, entities, rich_message) = self.text.write(client, self.parse_mode).await?;
        let request = functions::messages::EditMessage {
            message,
            reply_markup: self.reply_markup.as_ref().map(ReplyMarkup::write),
            entities,
            rich_message,
            ..edit_request(peer, self.message_id)
        };
        let (updates, request) = client.invoke_business(self.business_connection_id.as_deref(), request).await?;
        Message::from_updates(client, updates, chat_id, (request.message.unwrap_or_default(), request.entities))
    }
}

pub(super) fn edit_request(peer: base::InputPeer, id: i32) -> functions::messages::EditMessage {
    functions::messages::EditMessage {
        no_webpage: false,
        invert_media: false,
        peer,
        id,
        message: None,
        media: None,
        reply_markup: None,
        entities: None,
        schedule_date: None,
        schedule_repeat_period: None,
        quick_reply_shortcut_id: None,
        rich_message: None,
    }
}

caption_setters!(EditMessageText);
into_future!(EditMessageText, Message);

impl crate::methods::OnBehalf for EditMessageText<'_> {
    fn business_connection(&mut self) -> &mut Option<String> {
        &mut self.business_connection_id
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, types};
    use crate::types::InputRichMessage;

    #[tokio::test]
    async fn rich_message_replaces_text() {
        let (client, mut rx) = Client::recording();
        assert!(client.edit_message_text("me", 5, InputRichMessage::markdown("_x_")).await.is_err());
        let body = rx.try_recv().expect("no request sent");
        let flags = u32::from_le_bytes(body[4..8].try_into().unwrap());
        assert_eq!((flags & 1 << 11, flags & 1 << 3, flags & 1 << 23 != 0), (0, 0, true));
        let markdown =
            types::InputRichMessageMarkdown { rtl: false, noautolink: false, markdown: "_x_".into(), files: None };
        assert!(body.ends_with(&base::InputRichMessage::InputRichMessageMarkdown(Box::new(markdown)).to_bytes()));

        assert!(client.edit_message_text("me", 5, "hi").await.is_err());
        let flags = u32::from_le_bytes(rx.try_recv().unwrap()[4..8].try_into().unwrap());
        assert_eq!((flags & 1 << 11 != 0, flags & 1 << 23), (true, 0));
    }

    #[tokio::test]
    async fn business_edits_go_through_the_connection() {
        let (client, mut rx) = Client::recording();
        assert!(client.edit_message_text("me", 5, "hi").business_connection_id("c").await.is_err());
        let body = rx.try_recv().expect("no request sent");
        assert_eq!(body[..4], 0xdd289f8e_u32.to_le_bytes());
    }
}
