use super::send_options::{SendOptions, caption_setters, send_builder};
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::raw::base;
use crate::types::{Message, PeerRef, Text};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendMessage<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    text: Text,
    parse_mode: Option<ParseMode>,
    options: SendOptions,
}

impl Client {
    pub fn send_message(&self, chat_id: impl Into<PeerRef>, text: impl Into<Text>) -> SendMessage<'_> {
        SendMessage {
            client: self,
            chat_id: chat_id.into(),
            text: text.into(),
            parse_mode: None,
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
        let text = text.parsed(mode.unwrap_or(self.0.parse_mode)).trimmed();
        let mut entities = Vec::new();
        for mut entity in text.write_entities() {
            if let base::MessageEntity::InputMessageEntityMentionName(m) = &mut entity {
                if let base::InputUser::InputUser(u) = &m.user_id {
                    // As Pyrogram does: a user this account has never met can't be
                    // mentioned, so the mention is dropped and its text kept, rather
                    // than the whole message failing (a helper bot captioning a file
                    // with the requester's `tg://user` link).
                    match self.resolve_user(&u.user_id.into()).await {
                        Ok(user) => m.user_id = user,
                        Err(Error::PeerNotFound(_)) => continue,
                        Err(e) if e.is("USER_ID_INVALID") || e.is("PEER_ID_INVALID") => continue,
                        Err(e) => return Err(e),
                    }
                }
            }
            entities.push(entity);
        }
        Ok((text.text, entities))
    }
}

impl SendMessage<'_> {
    /// Sends the text without a link preview (Pyrogram's `disable_web_page_preview`).
    pub fn disable_web_page_preview(mut self) -> Self {
        self.options.no_webpage = true;
        self
    }

    async fn send(self) -> Result<Message> {
        let (message, entities) = self.client.write_text(self.text, self.parse_mode).await?;
        let text = (message, (!entities.is_empty()).then_some(entities));
        self.client.send_parsed(self.chat_id, None, text, None, self.options).await
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
    async fn other_errors_still_fail() {
        let (client, _sent) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(420, "FLOOD_WAIT_3")))]);
        assert!(client.write_text(HTML.into(), Some(ParseMode::Html)).await.is_err());
    }
}
