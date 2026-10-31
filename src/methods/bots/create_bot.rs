use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{PeerRef, User};

#[must_use = "a request does nothing until it is awaited"]
pub struct CreateBot<'a> {
    client: &'a Client,
    manager_bot_id: PeerRef,
    name: String,
    username: String,
    via_link: bool,
}

impl Client {
    pub fn create_bot(
        &self,
        manager_bot_id: impl Into<PeerRef>,
        name: impl Into<String>,
        username: impl Into<String>,
    ) -> CreateBot<'_> {
        CreateBot {
            client: self,
            manager_bot_id: manager_bot_id.into(),
            name: name.into(),
            username: username.into(),
            via_link: false,
        }
    }
}

impl CreateBot<'_> {
    pub fn via_link(mut self) -> Self {
        self.via_link = true;
        self
    }

    async fn send(self) -> Result<User> {
        let request = functions::bots::CreateBot {
            via_deeplink: self.via_link,
            name: self.name,
            username: self.username,
            manager_id: self.client.resolve_user(&self.manager_bot_id).await?,
        };
        Ok(User::from_raw(self.client, self.client.invoke(&request).await?))
    }
}

into_future!(CreateBot, User);

#[cfg(test)]
pub(super) mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Serializable, base, functions, types};
    use crate::utils::get_channel_id;

    /// A user as Telegram sends it, with an access hash and the bot flag when asked.
    pub(crate) fn user(id: i64, bot: bool) -> base::User {
        let mut buf = Vec::new();
        (1u32 | (u32::from(bot) << 14)).serialize(&mut buf);
        0u32.serialize(&mut buf);
        id.serialize(&mut buf);
        5i64.serialize(&mut buf);
        if bot {
            // bot_info_version shares the bot flag's bit.
            1i32.serialize(&mut buf);
        }
        base::User::User(Box::new(types::User::deserialize_bare(&mut &buf[..]).unwrap()))
    }

    fn seeded(replies: Vec<crate::Result<Vec<u8>>>) -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let (client, sent) = Client::scripted(replies);
        let mut peers = client.0.peers.lock().unwrap();
        let bot = types::InputPeerUser { user_id: 42, access_hash: 7 };
        peers.insert(42, base::InputPeer::InputPeerUser(Box::new(bot)));
        let channel = types::InputPeerChannel { channel_id: 9, access_hash: 3 };
        peers.insert(get_channel_id(9), base::InputPeer::InputPeerChannel(Box::new(channel)));
        drop(peers);
        (client, sent)
    }

    fn request(via_deeplink: bool) -> Vec<u8> {
        let manager_id = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 42, access_hash: 7 }));
        functions::bots::CreateBot { via_deeplink, name: "Name".into(), username: "name_bot".into(), manager_id }
            .to_bytes()
    }

    #[tokio::test]
    async fn creates_the_bot_under_its_manager() {
        let (client, mut sent) = seeded(vec![Ok(user(77, true).to_bytes()), Ok(user(78, true).to_bytes())]);
        let bot = client.create_bot(42, "Name", "name_bot").await.unwrap();
        assert_eq!(sent.recv().await.unwrap(), request(false));
        assert!(bot.id == 77 && bot.is_bot);
        assert!(client.0.peers.lock().unwrap().contains_key(&77));

        client.create_bot(42, "Name", "name_bot").via_link().await.unwrap();
        assert_eq!(sent.recv().await.unwrap(), request(true));
    }

    #[tokio::test]
    async fn a_channel_cannot_manage_bots() {
        let (client, mut sent) = seeded(Vec::new());
        let result = client.create_bot(get_channel_id(9), "Name", "name_bot").await;
        assert!(matches!(result, Err(Error::PeerNotFound(_))), "{result:?}");
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn errors_come_back_unchanged() {
        let (client, _sent) = seeded(vec![Err(Error::Rpc(RpcError::new(400, "MANAGER_PERMISSION_MISSING")))]);
        let result = client.create_bot(42, "Name", "name_bot").await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.is("MANAGER_PERMISSION_MISSING")), "{result:?}");
    }
}
