use crate::Client;
use crate::crypto::random;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::{Message, PeerRef};

impl Client {
    /// Starts a bot with a deep-link parameter, in its private chat or in a group (`chat_id`).
    pub async fn start_bot(
        &self,
        bot_id: impl Into<PeerRef>,
        chat_id: impl Into<PeerRef>,
        start_param: &str,
    ) -> Result<Message> {
        if start_param.is_empty() {
            return Err(Error::InvalidArgument("start_bot takes a non-empty start parameter"));
        }
        let chat_id = self.peer_id(chat_id.into()).await?;
        let request = functions::messages::StartBot {
            bot: self.resolve_user(&bot_id.into()).await?,
            peer: self.resolve_peer(chat_id).await?,
            random_id: i64::from_le_bytes(random()),
            start_param: start_param.into(),
        };
        let updates = self.invoke(&request).await?;
        Message::from_updates(self, updates, chat_id, (format!("/start {start_param}"), None))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;

    #[tokio::test]
    async fn an_empty_start_parameter_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        assert!(matches!(client.start_bot(42, 42, "").await, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }
}
