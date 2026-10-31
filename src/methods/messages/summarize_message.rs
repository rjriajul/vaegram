use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{FormattedText, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SummarizeMessage<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    to_language_code: Option<String>,
    tone: Option<String>,
}

impl Client {
    pub fn summarize_message(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> SummarizeMessage<'_> {
        SummarizeMessage { client: self, chat_id: chat_id.into(), message_id, to_language_code: None, tone: None }
    }
}

impl SummarizeMessage<'_> {
    pub fn to_language_code(mut self, language_code: impl Into<String>) -> Self {
        self.to_language_code = Some(language_code.into());
        self
    }

    pub fn tone(mut self, tone: impl Into<String>) -> Self {
        self.tone = Some(tone.into());
        self
    }

    async fn send(self) -> Result<FormattedText> {
        let request = functions::messages::SummarizeText {
            peer: self.client.resolve_peer(self.chat_id).await?,
            id: self.message_id,
            to_lang: self.to_language_code,
            tone: self.tone,
        };
        Ok(FormattedText::parse(self.client.invoke(&request).await?))
    }
}

into_future!(SummarizeMessage, FormattedText);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn summarize_sends_language_and_tone() {
        let reply = || {
            let text = types::TextWithEntities { text: "Short".into(), entities: Vec::new() };
            Ok(base::TextWithEntities::TextWithEntities(Box::new(text)).to_bytes())
        };
        let (client, mut rx) = Client::scripted(vec![reply(), reply()]);
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 2 }));
        client.0.peers.lock().unwrap().insert(7, peer.clone());
        let request = |to_lang: Option<&str>, tone: Option<&str>| {
            let (to_lang, tone) = (to_lang.map(String::from), tone.map(String::from));
            functions::messages::SummarizeText { peer: peer.clone(), id: 5, to_lang, tone }.to_bytes()
        };

        let summary = client.summarize_message(7, 5).to_language_code("es").tone("casual").await.unwrap();
        assert_eq!((summary.text.as_str(), summary.entities), ("Short", None));
        assert_eq!(rx.try_recv().unwrap(), request(Some("es"), Some("casual")));
        client.summarize_message(7, 5).await.unwrap();
        assert_eq!(rx.try_recv().unwrap(), request(None, None));
    }
}
