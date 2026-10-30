use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::{PeerRef, TranslatedText};

impl Client {
    pub async fn translate_message_text(
        &self,
        to_language_code: impl Into<String>,
        chat_id: impl Into<PeerRef>,
        message_ids: &[i32],
    ) -> Result<Vec<TranslatedText>> {
        if message_ids.is_empty() {
            return Err(Error::InvalidArgument("translate_message_text takes at least one message id"));
        }
        let request = functions::messages::TranslateText {
            peer: Some(self.resolve_peer(chat_id).await?),
            id: Some(message_ids.to_vec()),
            text: None,
            to_lang: to_language_code.into(),
            tone: None,
        };
        Ok(TranslatedText::parse_all(self.invoke(&request).await?))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;

    #[tokio::test]
    async fn no_ids_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        assert!(matches!(client.translate_message_text("es", -5, &[]).await, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }
}
