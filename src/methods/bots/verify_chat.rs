use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
pub struct VerifyChat<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    custom_description: Option<String>,
}

impl Client {
    pub fn verify_chat(&self, chat_id: impl Into<PeerRef>) -> VerifyChat<'_> {
        VerifyChat { client: self, chat_id: chat_id.into(), custom_description: None }
    }

    pub(crate) async fn set_custom_verification(
        &self,
        chat_id: PeerRef,
        enabled: bool,
        custom_description: Option<String>,
    ) -> Result<bool> {
        let custom_description = custom_description.map(|text| {
            base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities { text, entities: Vec::new() }))
        });
        let request = functions::bots::SetCustomVerification {
            enabled,
            bot: None,
            peer: self.resolve_peer(chat_id).await?,
            custom_description,
        };
        self.invoke(&request).await
    }
}

impl VerifyChat<'_> {
    /// Replaces the organization's default verification description, up to 70 characters.
    pub fn custom_description(mut self, text: impl Into<String>) -> Self {
        self.custom_description = Some(text.into());
        self
    }

    async fn send(self) -> Result<bool> {
        self.client.set_custom_verification(self.chat_id, true, self.custom_description).await
    }
}

into_future!(VerifyChat, bool);
