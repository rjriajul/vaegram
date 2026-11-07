use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::{Message, PeerRef};

impl Client {
    /// Answers a bot's "request users" or "request chat" reply keyboard button by sharing `peers` with the bot, and
    /// returns the service message that records the share.
    ///
    /// `message_id` is the bot message that carries the keyboard, and `button_id` the `request_id` of the pressed
    /// button ([`KeyboardButtonRequestUsers`](crate::types::KeyboardButtonRequestUsers) or
    /// [`KeyboardButtonRequestChat`](crate::types::KeyboardButtonRequestChat)). Fails with
    /// [`Error::InvalidArgument`] when `peers` is empty, and with a Telegram error when the peers don't match what
    /// the button asks for.
    pub async fn send_bot_requested_peer(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        button_id: i32,
        peers: impl IntoIterator<Item = impl Into<PeerRef>>,
    ) -> Result<Message> {
        let requested_peers = self.resolve_peers(peers).await?;
        if requested_peers.is_empty() {
            return Err(Error::InvalidArgument("share at least one chat"));
        }
        let chat_id = self.peer_id(chat_id.into()).await?;
        let request = functions::messages::SendBotRequestedPeer {
            peer: self.resolve_peer(chat_id).await?,
            msg_id: Some(message_id),
            webapp_req_id: None,
            button_id,
            requested_peers,
        };
        let updates = self.invoke(&request).await?;
        Message::from_updates(self, updates, Message::request(chat_id, String::new(), None))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::ServiceAction;
    use crate::utils::get_peer;

    const BOT: i64 = 42;
    const FRIEND: i64 = 7;

    fn user(user_id: i64) -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id, access_hash: user_id + 1 }))
    }

    #[tokio::test]
    async fn shares_the_chosen_peers_with_the_bot_and_returns_the_message() {
        // Telegram answers with the service message that records the share, in the bot's chat.
        let action = types::MessageActionRequestedPeer { button_id: 1, peers: vec![get_peer(FRIEND), get_peer(-9)] };
        let service = types::MessageService {
            out: true,
            mentioned: false,
            media_unread: false,
            reactions_are_possible: false,
            silent: false,
            post: false,
            legacy: false,
            id: 6,
            from_id: None,
            peer_id: get_peer(BOT),
            saved_peer_id: None,
            reply_to: None,
            date: 1,
            action: base::MessageAction::MessageActionRequestedPeer(Box::new(action)),
            reactions: None,
            ttl_period: None,
        };
        let message = base::Message::MessageService(Box::new(service));
        let update = types::UpdateNewMessage { message, pts: 1, pts_count: 1 };
        let update = base::Update::UpdateNewMessage(Box::new(update));
        let updates = types::Updates { updates: vec![update], users: Vec::new(), chats: Vec::new(), date: 1, seq: 0 };
        let (client, mut rx) = Client::scripted(vec![Ok(base::Updates::Updates(Box::new(updates)).to_bytes())]);
        for id in [BOT, FRIEND] {
            client.0.peers.lock().unwrap().insert(id, user(id));
        }
        let message = client.send_bot_requested_peer(BOT, 5, 1, [FRIEND, -9]).await.unwrap();
        assert_eq!((message.id, message.chat.id), (6, BOT));
        let shared =
            matches!(&message.service, Some(ServiceAction::ChatShared(c)) if (c.request_id, c.chat_id) == (1, -9));
        assert!(shared, "{:?}", message.service);
        let request = functions::messages::SendBotRequestedPeer {
            peer: user(BOT),
            msg_id: Some(5),
            webapp_req_id: None,
            button_id: 1,
            requested_peers: vec![user(FRIEND), base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 9 })],
        };
        assert_eq!(rx.try_recv().expect("no request sent"), request.to_bytes());
    }

    #[tokio::test]
    async fn refuses_an_empty_choice() {
        let (client, mut rx) = Client::recording();
        let none: [i64; 0] = [];
        let result = client.send_bot_requested_peer(BOT, 5, 1, none).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(rx.try_recv().is_err());
    }
}
