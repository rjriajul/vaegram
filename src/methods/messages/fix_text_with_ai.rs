use super::compose_text_with_ai::ComposeTextWithAi;
use crate::Client;
use crate::types::Text;

impl Client {
    /// Corrects spelling and grammar; [`ComposedText::diff`](crate::types::ComposedText::diff) marks each fix.
    pub fn fix_text_with_ai(&self, text: impl Into<Text>) -> ComposeTextWithAi<'_> {
        ComposeTextWithAi::new(self, text.into(), true)
    }
}

#[cfg(test)]
mod tests {
    use super::super::compose_text_with_ai::tests::{composed, text};
    use crate::Client;
    use crate::raw::{Serializable, functions};

    #[tokio::test]
    async fn fix_only_proofreads() {
        let (client, mut rx) = Client::scripted(vec![Ok(composed())]);
        client.fix_text_with_ai("Ths is a tset").await.unwrap();
        let expected = functions::messages::ComposeMessageWithAI {
            proofread: true,
            emojify: false,
            text: text("Ths is a tset", Vec::new()),
            translate_to_lang: None,
            tone: None,
        };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }
}
