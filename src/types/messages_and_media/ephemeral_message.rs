use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::methods::{EditEphemeralMessage, SendEphemeralMessage};
use crate::raw::{base, types};
use crate::types::{Chat, MessageText, User};
use crate::utils::get_peer;

#[derive(Debug, Clone)]
pub struct EphemeralMessage {
    /// The ephemeral message id; it has its own sequence, separate from regular message ids.
    pub id: i32,
    pub chat: Chat,
    pub from_user: Option<User>,
    pub sender_chat: Option<Chat>,
    pub receiver_user: User,
    pub date: i32,
    pub text: String,
    pub outgoing: bool,
    pub raw: Box<types::EphemeralMessage>,
    client: Client,
}

impl EphemeralMessage {
    pub(crate) fn parse(client: &Client, raw: base::EphemeralMessage, entities: &Entities) -> EphemeralMessage {
        let base::EphemeralMessage::EphemeralMessage(raw) = raw;
        let (from_user, sender_chat) = match &raw.from_id {
            base::Peer::PeerUser(p) => (Some(User::parse(p.user_id, entities)), None),
            peer => (None, Some(Chat::parse(peer, entities, false))),
        };
        let chat = match &raw.peer_id {
            Some(peer) => Chat::parse(peer, entities, false),
            // Without a peer the message lives in the private chat between the bot and the other side.
            None if raw.out => Chat::parse(&get_peer(raw.receiver_id), entities, false),
            None => Chat::parse(&raw.from_id, entities, false),
        };
        EphemeralMessage {
            id: raw.id,
            chat,
            from_user,
            sender_chat,
            receiver_user: User::parse(raw.receiver_id, entities),
            date: raw.date,
            text: raw.message.clone(),
            outgoing: raw.out,
            raw,
            client: client.clone(),
        }
    }

    pub(crate) fn from_updates(client: &Client, updates: base::Updates) -> Result<EphemeralMessage> {
        let (updates, users, chats) = match updates {
            base::Updates::Updates(u) => (u.updates, u.users, u.chats),
            base::Updates::UpdatesCombined(u) => (u.updates, u.users, u.chats),
            base::Updates::UpdateShort(u) => (vec![u.update], Vec::new(), Vec::new()),
            _ => return Err(Error::Unexpected("Telegram sent no ephemeral message back")),
        };
        let entities = Entities::new(client, users, chats);
        updates
            .into_iter()
            .find_map(|u| match u {
                base::Update::UpdateNewEphemeralMessage(u) => Some(u.message),
                base::Update::UpdateEditEphemeralMessage(u) => Some(u.message),
                _ => None,
            })
            .map(|m| EphemeralMessage::parse(client, m, &entities))
            .ok_or(Error::Unexpected("Telegram sent no ephemeral message back"))
    }

    pub fn edit_text(&self, text: impl Into<MessageText>) -> EditEphemeralMessage<'_> {
        self.client.edit_ephemeral_message_text(self.chat.id, self.receiver_user.id, self.id, text)
    }

    pub async fn delete(&self) -> Result<bool> {
        self.client.delete_ephemeral_message(self.chat.id, self.receiver_user.id, self.id).await
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn reply(&self, text: impl Into<String>) -> SendEphemeralMessage<'_> {
        let receiver = self.from_user.as_ref().filter(|_| !self.outgoing).map_or(self.receiver_user.id, |u| u.id);
        self.client.send_ephemeral_message(self.chat.id, receiver, text).reply_to_ephemeral(self.id)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::EphemeralMessage;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};
    use crate::utils::get_channel_id;

    pub(crate) fn ephemeral(out: bool, peer_id: Option<base::Peer>) -> base::EphemeralMessage {
        base::EphemeralMessage::EphemeralMessage(Box::new(types::EphemeralMessage {
            out,
            welcome_template: false,
            invert_media: false,
            noforwards: false,
            id: 7,
            from_id: base::Peer::PeerUser(Box::new(types::PeerUser { user_id: 5 })),
            peer_id,
            receiver_id: 1000,
            top_msg_id: None,
            date: 1,
            message: "secret".into(),
            entities: None,
            media: None,
            reply_markup: None,
            reply_to: None,
            rich_message: None,
            chat_instance: None,
            anchor_msg_id: None,
        }))
    }

    pub(crate) fn channel() -> base::Peer {
        base::Peer::PeerChannel(Box::new(types::PeerChannel { channel_id: 42 }))
    }

    #[test]
    fn parses_ephemeral_messages() {
        let m = EphemeralMessage::parse(&Client::offline(), ephemeral(false, Some(channel())), &Entities::default());
        assert_eq!((m.id, m.chat.id, m.receiver_user.id, m.text.as_str()), (7, get_channel_id(42), 1000, "secret"));
        assert_eq!((m.from_user.as_ref().map(|u| u.id), m.outgoing), (Some(5), false));

        let m = EphemeralMessage::parse(&Client::offline(), ephemeral(true, None), &Entities::default());
        assert_eq!(m.chat.id, 1000);
    }
}
