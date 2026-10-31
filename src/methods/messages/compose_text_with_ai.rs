use super::send_options::caption_setters;
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{ComposedText, Text};

#[must_use = "a request does nothing until it is awaited"]
pub struct ComposeTextWithAi<'a> {
    client: &'a Client,
    text: Text,
    parse_mode: Option<ParseMode>,
    to_language_code: Option<String>,
    tone: Option<String>,
    emojify: bool,
    proofread: bool,
}

impl Client {
    /// Rewrites `text`: translated, in another tone, or with emojis added.
    pub fn compose_text_with_ai(&self, text: impl Into<Text>) -> ComposeTextWithAi<'_> {
        ComposeTextWithAi::new(self, text.into(), false)
    }
}

impl<'a> ComposeTextWithAi<'a> {
    pub(super) fn new(client: &'a Client, text: Text, proofread: bool) -> Self {
        ComposeTextWithAi {
            client,
            text,
            parse_mode: None,
            to_language_code: None,
            tone: None,
            emojify: false,
            proofread,
        }
    }
}

impl ComposeTextWithAi<'_> {
    pub fn to_language_code(mut self, language_code: impl Into<String>) -> Self {
        self.to_language_code = Some(language_code.into());
        self
    }

    pub fn tone(mut self, tone: impl Into<String>) -> Self {
        self.tone = Some(tone.into());
        self
    }

    pub fn add_emojis(mut self) -> Self {
        self.emojify = true;
        self
    }

    async fn send(self) -> Result<ComposedText> {
        let (text, entities) = self.client.write_text(self.text, self.parse_mode).await?;
        let tone = self.tone.map(|tone| {
            base::InputAiComposeTone::InputAiComposeToneDefault(Box::new(types::InputAiComposeToneDefault { tone }))
        });
        let request = functions::messages::ComposeMessageWithAI {
            proofread: self.proofread,
            emojify: self.emojify,
            text: base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities { text, entities })),
            translate_to_lang: self.to_language_code,
            tone,
        };
        Ok(ComposedText::parse(self.client.invoke(&request).await?))
    }
}

caption_setters!(ComposeTextWithAi);
into_future!(ComposeTextWithAi, ComposedText);

#[cfg(test)]
pub(super) mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Serializable, base, functions, types};

    pub(crate) fn composed() -> Vec<u8> {
        let text = types::TextWithEntities { text: "Hallo".into(), entities: Vec::new() };
        let result_text = base::TextWithEntities::TextWithEntities(Box::new(text));
        let composed = types::messages::ComposedMessageWithAI { result_text, diff_text: None };
        base::messages::ComposedMessageWithAI::ComposedMessageWithAI(Box::new(composed)).to_bytes()
    }

    pub(crate) fn text(text: &str, entities: Vec<base::MessageEntity>) -> base::TextWithEntities {
        base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities { text: text.into(), entities }))
    }

    #[tokio::test]
    async fn compose_sends_its_options() {
        let (client, mut rx) = Client::scripted(vec![Ok(composed())]);
        let composed = client.compose_text_with_ai("<b>hi</b>").html().to_language_code("de").tone("formal");
        let composed = composed.add_emojis().await.unwrap();
        assert_eq!((composed.text.text.as_str(), composed.diff), ("Hallo", None));
        let bold = base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 2 });
        let tone = types::InputAiComposeToneDefault { tone: "formal".into() };
        let expected = functions::messages::ComposeMessageWithAI {
            proofread: false,
            emojify: true,
            text: text("hi", vec![bold]),
            translate_to_lang: Some("de".into()),
            tone: Some(base::InputAiComposeTone::InputAiComposeToneDefault(Box::new(tone))),
        };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn ai_errors_pass_through() {
        let refused = || Err(Error::Rpc(RpcError::new(400, "PREMIUM_ACCOUNT_REQUIRED")));
        let (client, _rx) = Client::scripted(vec![refused(), refused()]);
        let results = [client.compose_text_with_ai("hi").await, client.fix_text_with_ai("hi").await];
        for result in results {
            assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "PREMIUM_ACCOUNT_REQUIRED"), "{result:?}");
        }
    }
}
