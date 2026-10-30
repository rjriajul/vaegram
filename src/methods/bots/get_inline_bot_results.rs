use crate::Client;
use crate::errors::Result;
use crate::methods::messages::geo_point;
use crate::raw::{base, functions};
use crate::types::{InlineBotResults, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct GetInlineBotResults<'a> {
    client: &'a Client,
    bot_id: PeerRef,
    query: String,
    chat_id: Option<PeerRef>,
    offset: String,
    location: Option<(f64, f64)>,
}

impl Client {
    pub fn get_inline_bot_results(
        &self,
        bot_id: impl Into<PeerRef>,
        query: impl Into<String>,
    ) -> GetInlineBotResults<'_> {
        GetInlineBotResults {
            client: self,
            bot_id: bot_id.into(),
            query: query.into(),
            chat_id: None,
            offset: String::new(),
            location: None,
        }
    }
}

impl GetInlineBotResults<'_> {
    /// The chat the query is typed in; Saved Messages by default.
    pub fn chat_id(mut self, chat_id: impl Into<PeerRef>) -> Self {
        self.chat_id = Some(chat_id.into());
        self
    }

    pub fn offset(mut self, offset: impl Into<String>) -> Self {
        self.offset = offset.into();
        self
    }

    pub fn location(mut self, latitude: f64, longitude: f64) -> Self {
        self.location = Some((latitude, longitude));
        self
    }

    async fn send(self) -> Result<InlineBotResults> {
        let client = self.client;
        let peer = match self.chat_id {
            Some(chat_id) => client.resolve_peer(chat_id).await?,
            None => base::InputPeer::InputPeerSelf,
        };
        let request = functions::messages::GetInlineBotResults {
            bot: client.resolve_user(&self.bot_id).await?,
            peer,
            geo_point: self.location.map(|(lat, long)| geo_point(lat, long, None)),
            query: self.query,
            offset: self.offset,
        };
        Ok(InlineBotResults::parse(client.invoke(&request).await?))
    }
}

into_future!(GetInlineBotResults, InlineBotResults);
