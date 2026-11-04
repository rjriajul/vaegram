use super::send_options::{SendOptions, caption_setters, send_builder};
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::base;
use crate::types::{LinkPreviewOptions, Message, PeerRef, Text};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendMessage<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    text: Text,
    parse_mode: Option<ParseMode>,
    link_preview_options: Option<LinkPreviewOptions>,
    options: SendOptions,
}

impl Client {
    pub fn send_message(&self, chat_id: impl Into<PeerRef>, text: impl Into<Text>) -> SendMessage<'_> {
        SendMessage {
            client: self,
            chat_id: chat_id.into(),
            text: text.into(),
            parse_mode: None,
            link_preview_options: None,
            options: SendOptions::default(),
        }
    }

    pub(crate) async fn parse_text(
        &self,
        text: &str,
        mode: Option<ParseMode>,
    ) -> Result<(String, Vec<base::MessageEntity>)> {
        self.write_text(text.into(), mode).await
    }

    pub(crate) async fn write_text(
        &self,
        text: Text,
        mode: Option<ParseMode>,
    ) -> Result<(String, Vec<base::MessageEntity>)> {
        let text = text.parsed(mode.unwrap_or_else(|| self.parse_mode())).trimmed();
        let entities = self.resolve_mentions(text.write_entities()).await?;
        Ok((text.text, entities))
    }
}

impl SendMessage<'_> {
    /// Sends the text without a link preview (Pyrogram's `disable_web_page_preview`).
    pub fn disable_web_page_preview(mut self) -> Self {
        self.options.no_webpage = true;
        self
    }

    /// Which link to preview, how, and whether to show it above the text. Replaces the client default.
    pub fn link_preview_options(mut self, options: LinkPreviewOptions) -> Self {
        self.link_preview_options = Some(options);
        self
    }

    async fn send(mut self) -> Result<Message> {
        let preview = self.client.link_preview(self.link_preview_options.as_ref()).map(|o| o.resolve());
        let (message, entities) = self.client.write_text(self.text, self.parse_mode).await?;
        let text = (message, (!entities.is_empty()).then_some(entities));
        let mut media = None;
        if let Some(preview) = preview {
            self.options.no_webpage |= preview.no_webpage;
            self.options.invert_media = preview.invert_media;
            // A disabled preview or an explicit `disable_web_page_preview` sends no web page media.
            if !self.options.no_webpage {
                media = preview.media;
            }
        }
        self.client.send_parsed(self.chat_id, media, text, None, self.options).await
    }
}

caption_setters!(SendMessage);
send_builder!(SendMessage);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::ParseMode;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Serializable, base, types};

    const HTML: &str = "<b>hi</b> <a href=\"tg://user?id=42\">you</a>";

    fn kinds(entities: &[base::MessageEntity]) -> Vec<&'static str> {
        entities
            .iter()
            .map(|e| match e {
                base::MessageEntity::MessageEntityBold(_) => "bold",
                base::MessageEntity::InputMessageEntityMentionName(_) => "mention",
                _ => "other",
            })
            .collect()
    }

    #[tokio::test]
    async fn a_known_user_is_mentioned() {
        let (client, _sent) = Client::scripted(Vec::new());
        let user = types::InputPeerUser { user_id: 42, access_hash: 7 };
        client.0.peers.lock().unwrap().insert(42, base::InputPeer::InputPeerUser(Box::new(user)));
        let (text, entities) = client.write_text(HTML.into(), Some(ParseMode::Html)).await.unwrap();
        assert_eq!((text.as_str(), kinds(&entities)), ("hi you", vec!["bold", "mention"]));
    }

    #[tokio::test]
    async fn an_unknown_user_keeps_the_text_but_not_the_mention() {
        let no_users: Vec<base::User> = Vec::new();
        let (client, _sent) = Client::scripted(vec![Ok(no_users.to_bytes())]);
        let (text, entities) = client.write_text(HTML.into(), Some(ParseMode::Html)).await.unwrap();
        assert_eq!((text.as_str(), kinds(&entities)), ("hi you", vec!["bold"]));

        let refused = Err(Error::Rpc(RpcError::new(400, "USER_ID_INVALID")));
        let (client, _sent) = Client::scripted(vec![refused]);
        let (_, entities) = client.write_text(HTML.into(), Some(ParseMode::Html)).await.unwrap();
        assert_eq!(kinds(&entities), vec!["bold"]);
    }

    #[tokio::test]
    async fn a_repeated_unknown_user_is_looked_up_once() {
        let html = "<a href=\"tg://user?id=42\">a</a> <a href=\"tg://user?id=42\">b</a>";
        let no_users: Vec<base::User> = Vec::new();
        let (client, mut sent) = Client::scripted(vec![Ok(no_users.to_bytes())]);
        let (_, entities) = client.write_text(html.into(), Some(ParseMode::Html)).await.unwrap();
        assert!(entities.is_empty());
        assert!(sent.try_recv().is_ok() && sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn other_errors_still_fail() {
        let (client, _sent) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(420, "FLOOD_WAIT_3")))]);
        assert!(client.write_text(HTML.into(), Some(ParseMode::Html)).await.is_err());
    }
}
