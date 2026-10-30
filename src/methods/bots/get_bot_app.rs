use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::{BotApp, PeerRef};

impl Client {
    pub async fn get_bot_app(&self, bot_id: impl Into<PeerRef>, short_name: impl Into<String>) -> Result<BotApp> {
        let short_name = short_name.into();
        if short_name.is_empty() {
            return Err(Error::InvalidArgument("get_bot_app takes a non-empty short name"));
        }
        let bot_id = self.resolve_user(&bot_id.into()).await?;
        let app = types::InputBotAppShortName { bot_id, short_name };
        let app = base::InputBotApp::InputBotAppShortName(Box::new(app));
        BotApp::parse(self.invoke(&functions::messages::GetBotApp { app, hash: 0 }).await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{base, types};

    #[tokio::test]
    async fn an_empty_short_name_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        let peer = types::InputPeerUser { user_id: 42, access_hash: 7 };
        client.0.peers.lock().unwrap().insert(42, base::InputPeer::InputPeerUser(Box::new(peer)));
        assert!(matches!(client.get_bot_app(42, "").await, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }
}
