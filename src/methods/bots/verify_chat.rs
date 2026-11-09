use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::methods::accepted;
use crate::methods::messages::caption_setters;
use crate::raw::{base, functions, types};
use crate::types::{PeerRef, Text};

/// Request built by [`Client::verify_chat`].
#[must_use = "a request does nothing until it is awaited"]
pub struct VerifyChat<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    custom_description: Option<Text>,
    parse_mode: Option<ParseMode>,
}

impl Client {
    /// Verifies a chat on behalf of your organization.
    pub fn verify_chat(&self, chat_id: impl Into<PeerRef>) -> VerifyChat<'_> {
        VerifyChat { client: self, chat_id: chat_id.into(), custom_description: None, parse_mode: None }
    }

    pub(crate) async fn set_custom_verification(
        &self,
        chat_id: PeerRef,
        enabled: bool,
        custom_description: Option<base::TextWithEntities>,
    ) -> Result<()> {
        let request = functions::bots::SetCustomVerification {
            enabled,
            bot: None,
            peer: self.resolve_peer(chat_id).await?,
            custom_description,
        };
        accepted(self.invoke(&request).await?)
    }
}

impl VerifyChat<'_> {
    /// Replaces the organization's default verification description, up to 70 characters.
    ///
    /// The description is parsed with the client's default parse mode unless [`html`](VerifyChat::html),
    /// [`markdown`](VerifyChat::markdown) or [`parse_mode`](VerifyChat::parse_mode) is set.
    pub fn custom_description(mut self, text: impl Into<Text>) -> Self {
        self.custom_description = Some(text.into());
        self
    }

    async fn send(self) -> Result<()> {
        let custom_description = match self.custom_description {
            Some(text) => {
                let (text, entities) = self.client.write_text(text, self.parse_mode).await?;
                Some(base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities { text, entities })))
            }
            None => None,
        };
        self.client.set_custom_verification(self.chat_id, true, custom_description).await
    }
}

caption_setters!(VerifyChat);
into_future!(VerifyChat, ());

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn verify_chat_parses_an_html_description() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes())]);
        let channel = types::InputPeerChannel { channel_id: 123, access_hash: 456 };
        client.0.peers.lock().unwrap().insert(123, base::InputPeer::InputPeerChannel(Box::new(channel)));

        client.verify_chat(123).custom_description("<b>official</b>").html().await.unwrap();

        let bold = base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 8 });
        let desc = types::TextWithEntities { text: "official".into(), entities: vec![bold] };
        let expected = functions::bots::SetCustomVerification {
            enabled: true,
            bot: None,
            peer: base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel {
                channel_id: 123,
                access_hash: 456,
            })),
            custom_description: Some(base::TextWithEntities::TextWithEntities(Box::new(desc))),
        };
        assert_eq!(sent.recv().await.unwrap(), expected.to_bytes());
    }
}
