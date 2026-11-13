use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::methods::{EditEphemeralMessage, SendEphemeralMessage};
use crate::raw::{base, types};
use crate::types::{Chat, Media, MessageEntity, MessageText, ReplyMarkup, User};
use crate::utils::get_peer;

/// A message in a chat that only one user, the receiver, sees.
#[derive(Debug, Clone)]
pub struct EphemeralMessage {
    /// Id of the message. Ephemeral ids have their own sequence, separate from regular message ids.
    pub id: i32,
    /// The chat the message was sent in.
    pub chat: Chat,
    /// The sending user. `None` if a chat sent it.
    pub from_user: Option<User>,
    /// The sending chat. `None` if a user sent it.
    pub sender_chat: Option<Chat>,
    /// The only user who sees the message.
    pub receiver_user: User,
    /// When the message was sent, as a Unix timestamp.
    pub date: i32,
    /// Message text, or the caption when there is media.
    pub text: String,
    /// `true` if the current account sent the message.
    pub outgoing: bool,
    /// `true` if the message is the template a bot sends as the welcome message of its private chats.
    pub is_welcome_template: bool,
    /// Telegram's `anchor_msg_id`. The receiver's copy of a message sent with
    /// [`replace_callback_query_message`](SendEphemeralMessage::replace_callback_query_message) names the pressed
    /// message here; the copy `ephemeral.sendMessage` returned to the bot had `None` when checked on 2026-11-13.
    pub anchor_message_id: Option<i32>,
    /// Telegram's `top_msg_id`: the thread or forum topic the message is in, `None` when Telegram names none. In an
    /// answer to a button press it was the pressed message's id (checked on 2026-11-13, in a supergroup without
    /// topics, with no reply asked for), so it is not safe to pass back as a topic.
    pub message_thread_id: Option<i32>,
    /// Formatting of the text. `None` if it has none.
    pub entities: Option<Vec<MessageEntity>>,
    /// Attached media. `None` if there is none.
    pub media: Option<Media>,
    /// Id of the ephemeral message this one replies to. `None` if it isn't a reply.
    pub reply_to_message_id: Option<i32>,
    /// `true` if forwarding and saving the message is restricted.
    pub has_protected_content: bool,
    /// `true` if the caption is shown above the media.
    pub show_caption_above_media: bool,
    pub(crate) raw: Box<types::EphemeralMessage>,
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
            is_welcome_template: raw.welcome_template,
            anchor_message_id: raw.anchor_msg_id,
            message_thread_id: raw.top_msg_id,
            entities: raw.entities.as_deref().and_then(|e| MessageEntity::parse_all(e, entities)),
            media: raw.media.as_ref().and_then(|m| Media::parse(client, m, entities)),
            reply_to_message_id: match &raw.reply_to {
                Some(base::MessageReplyHeader::MessageReplyHeader(h)) => h.reply_to_msg_id,
                _ => None,
            },
            has_protected_content: raw.noforwards,
            show_caption_above_media: raw.invert_media,
            raw,
            client: client.clone(),
        }
    }

    /// Returns the message's keyboard. `None` if it has none.
    pub fn reply_markup(&self) -> Option<ReplyMarkup> {
        ReplyMarkup::parse(self.raw.reply_markup.as_ref()?)
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

    /// Edits the text of the message. Awaiting returns the edited [`EphemeralMessage`].
    pub fn edit_text(&self, text: impl Into<MessageText>) -> EditEphemeralMessage<'_> {
        self.client.edit_ephemeral_message_text(self.chat.id, self.receiver_user.id, self.id, text)
    }

    /// Deletes the message for its receiver.
    pub async fn delete(&self) -> Result<()> {
        self.client.delete_ephemeral_message(self.chat.id, self.receiver_user.id, self.id).await
    }

    /// Returns the client that received the message.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Sends an ephemeral reply to this message in the same chat.
    ///
    /// The reply goes to the sender of an incoming message, or to the same receiver for an outgoing one. The text is
    /// a [`MessageText`]: plain or formatted text, or a rich message. Awaiting returns the sent [`EphemeralMessage`].
    pub fn reply(&self, text: impl Into<MessageText>) -> SendEphemeralMessage<'_> {
        let receiver = self.from_user.as_ref().filter(|_| !self.outgoing).map_or(self.receiver_user.id, |u| u.id);
        self.client.send_ephemeral_message(self.chat.id, receiver, text).reply_to_ephemeral(self.id)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::message::tests::header;
    use super::EphemeralMessage;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};
    use crate::types::{Media, ReplyMarkup};
    use crate::utils::get_channel_id;

    pub(crate) fn ephemeral(out: bool, peer_id: Option<base::Peer>) -> base::EphemeralMessage {
        base::EphemeralMessage::EphemeralMessage(Box::new(types::EphemeralMessage {
            out,
            welcome_template: false,
            invert_media: false,
            noforwards: false,
            id: 7,
            from_id: base::Peer::PeerUser(types::PeerUser { user_id: 5 }),
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
        base::Peer::PeerChannel(types::PeerChannel { channel_id: 42 })
    }

    #[test]
    fn parses_ephemeral_messages() {
        let m = EphemeralMessage::parse(&Client::offline(), ephemeral(false, Some(channel())), &Entities::default());
        assert_eq!((m.id, m.chat.id, m.receiver_user.id, m.text.as_str()), (7, get_channel_id(42), 1000, "secret"));
        assert_eq!((m.from_user.as_ref().map(|u| u.id), m.outgoing), (Some(5), false));

        let m = EphemeralMessage::parse(&Client::offline(), ephemeral(true, None), &Entities::default());
        assert_eq!(m.chat.id, 1000);
    }

    #[test]
    fn ephemeral_reads_template_anchor_and_media() {
        let base::EphemeralMessage::EphemeralMessage(mut raw) = ephemeral(false, Some(channel()));
        raw.welcome_template = true;
        raw.noforwards = true;
        raw.invert_media = true;
        raw.anchor_msg_id = Some(11);
        raw.top_msg_id = Some(3);
        let bold = types::MessageEntityBold { offset: 0, length: 3 };
        raw.entities = Some(vec![base::MessageEntity::MessageEntityBold(bold)]);
        let dice = types::MessageMediaDice { value: 4, emoticon: "🎲".into(), game_outcome: None };
        raw.media = Some(base::MessageMedia::MessageMediaDice(Box::new(dice)));
        raw.reply_markup = Some(base::ReplyMarkup::ReplyKeyboardHide(types::ReplyKeyboardHide { selective: true }));
        let reply = types::MessageReplyHeader { reply_to_msg_id: Some(6), ..header() };
        raw.reply_to = Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(reply)));

        let raw = base::EphemeralMessage::EphemeralMessage(raw);
        let m = EphemeralMessage::parse(&Client::offline(), raw, &Entities::default());
        assert!(m.is_welcome_template && m.has_protected_content && m.show_caption_above_media);
        assert_eq!((m.anchor_message_id, m.message_thread_id, m.reply_to_message_id), (Some(11), Some(3), Some(6)));
        assert_eq!(m.entities.as_ref().map(Vec::len), Some(1));
        assert!(matches!(&m.media, Some(Media::Dice(d)) if d.value == 4));
        assert!(matches!(m.reply_markup(), Some(ReplyMarkup::Remove(r)) if r.selective));

        let plain = EphemeralMessage::parse(&Client::offline(), ephemeral(false, None), &Entities::default());
        assert!(!plain.is_welcome_template && plain.media.is_none() && plain.reply_markup().is_none());
    }
}
