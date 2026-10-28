use super::send_options::{SendOptions, send_builder};
use crate::Client;
use crate::errors::Result;
use crate::types::{InputRichMessage, Message, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendRichMessage<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    rich_message: InputRichMessage,
    options: SendOptions,
}

impl Client {
    pub fn send_rich_message(
        &self,
        chat_id: impl Into<PeerRef>,
        rich_message: InputRichMessage,
    ) -> SendRichMessage<'_> {
        SendRichMessage { client: self, chat_id: chat_id.into(), rich_message, options: SendOptions::default() }
    }
}

impl SendRichMessage<'_> {
    async fn send(self) -> Result<Message> {
        let rich_message = Some(self.rich_message.write());
        self.client.send_parsed(self.chat_id, None, (String::new(), None), rich_message, self.options).await
    }
}

send_builder!(SendRichMessage);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, types};
    use crate::types::InputRichMessage;

    async fn sent_body(rich: InputRichMessage) -> Vec<u8> {
        let (client, mut rx) = Client::recording();
        assert!(client.send_rich_message("me", rich).await.is_err());
        rx.try_recv().expect("no request sent")
    }

    #[tokio::test]
    async fn sets_rich_message() {
        let body = sent_body(InputRichMessage::html("<b>hi</b>").rtl()).await;
        let flags = u32::from_le_bytes(body[4..8].try_into().unwrap());
        assert_ne!(flags & 1 << 23, 0);
        let html = types::InputRichMessageHTML { rtl: true, noautolink: false, html: "<b>hi</b>".into(), files: None };
        assert!(body.ends_with(&base::InputRichMessage::InputRichMessageHTML(Box::new(html)).to_bytes()));

        let body = sent_body(InputRichMessage::markdown("**hi**").no_autolink()).await;
        let markdown =
            types::InputRichMessageMarkdown { rtl: false, noautolink: true, markdown: "**hi**".into(), files: None };
        assert!(body.ends_with(&base::InputRichMessage::InputRichMessageMarkdown(Box::new(markdown)).to_bytes()));
    }

    #[tokio::test]
    async fn short_reply_keeps_what_was_sent() {
        let short = base::Updates::UpdateShortSentMessage(Box::new(types::UpdateShortSentMessage {
            out: true,
            id: 9,
            pts: 0,
            pts_count: 0,
            date: 0,
            media: None,
            entities: None,
            ttl_period: None,
        }));
        let (client, _rx) = Client::scripted(vec![Ok(short.to_bytes())]);
        let message = client.send_rich_message("me", InputRichMessage::html("<h1>Hi</h1> <b>there</b>")).await.unwrap();
        assert_eq!((message.id, message.text.as_str(), message.rich_message), (9, "Hi there", None));
    }

    #[tokio::test]
    async fn business_send_keeps_the_connection() {
        let raw = crate::types::Message::bare(9, crate::utils::get_peer(5), 0);
        let update = types::UpdateBotNewBusinessMessage {
            connection_id: "conn".into(),
            message: base::Message::Message(Box::new(raw)),
            reply_to_message: None,
            qts: 0,
        };
        let updates = base::Updates::UpdateShort(Box::new(types::UpdateShort {
            update: base::Update::UpdateBotNewBusinessMessage(Box::new(update)),
            date: 0,
        }));
        let (client, _rx) = Client::scripted(vec![Ok(updates.to_bytes())]);
        let message =
            client.send_rich_message("me", InputRichMessage::html("hi")).business_connection_id("conn").await.unwrap();
        assert_eq!(message.business_connection_id.as_deref(), Some("conn"));
    }
}
