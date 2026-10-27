use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::ChatType;
use crate::errors::{Error, Result};
use crate::methods::{EditMessageText, SendMessage};
use crate::raw::{Serializable, base, types};
use crate::types::{Chat, User};
use crate::utils::get_peer;

#[derive(Clone)]
pub struct Message {
    pub id: i32,
    pub chat: Chat,
    pub from_user: Option<User>,
    pub sender_chat: Option<Chat>,
    pub date: i32,
    pub text: String,
    pub outgoing: bool,
    pub edit_date: Option<i32>,
    pub raw: Box<types::Message>,
    client: Client,
}

impl Message {
    pub(crate) fn parse(client: &Client, raw: &types::Message, entities: &Entities) -> Message {
        let chat = Chat::parse(&raw.peer_id, entities, raw.post);
        let (mut from_user, mut sender_chat) = (None, None);
        match (&raw.from_id, &raw.peer_id) {
            (Some(base::Peer::PeerUser(p)), _) => from_user = Some(User::parse(p.user_id, entities)),
            (Some(peer), _) => sender_chat = Some(Chat::parse(peer, entities, false)),
            (None, base::Peer::PeerUser(p)) => {
                let id = if raw.out { client.0.me.lock().unwrap().id } else { p.user_id };
                from_user = Some(User::parse(id, entities));
            }
            (None, _) if chat.kind == ChatType::Channel => sender_chat = Some(chat.clone()),
            (None, _) => {}
        }
        Message {
            id: raw.id,
            chat,
            from_user,
            sender_chat,
            date: raw.date,
            text: raw.message.clone(),
            outgoing: raw.out,
            edit_date: raw.edit_date,
            raw: Box::new(raw.clone()),
            client: client.clone(),
        }
    }

    pub(crate) fn from_updates(
        client: &Client,
        updates: base::Updates,
        chat_id: i64,
        sent: (String, Option<Vec<base::MessageEntity>>),
    ) -> Result<Message> {
        let (updates, users, chats) = match updates {
            base::Updates::UpdateShortSentMessage(u) => {
                let mut raw = bare_message(u.id, get_peer(chat_id), u.date)?;
                raw.out = u.out;
                raw.from_id = u.out.then(|| get_peer(client.0.me.lock().unwrap().id));
                (raw.message, raw.entities, raw.media) = (sent.0, u.entities.or(sent.1), u.media);
                return Ok(Message::parse(client, &raw, &Entities::default()));
            }
            base::Updates::Updates(u) => (u.updates, u.users, u.chats),
            base::Updates::UpdatesCombined(u) => (u.updates, u.users, u.chats),
            base::Updates::UpdateShort(u) => (vec![u.update], Vec::new(), Vec::new()),
            _ => return Err(Error::Unexpected("Telegram sent no message back")),
        };
        let entities = Entities::new(client, users, chats);
        updates
            .into_iter()
            .find_map(|u| match u {
                base::Update::UpdateNewMessage(u) => Some(u.message),
                base::Update::UpdateNewChannelMessage(u) => Some(u.message),
                base::Update::UpdateEditMessage(u) => Some(u.message),
                base::Update::UpdateEditChannelMessage(u) => Some(u.message),
                _ => None,
            })
            .and_then(|m| match m {
                base::Message::Message(m) => Some(Message::parse(client, &m, &entities)),
                _ => None,
            })
            .ok_or(Error::Unexpected("Telegram sent no message back"))
    }

    pub fn edit_text(&self, text: impl Into<String>) -> EditMessageText<'_> {
        self.client.edit_message_text(self.chat.id, self.id, text)
    }

    pub async fn delete(&self) -> Result<()> {
        self.client.delete_messages(self.chat.id, &[self.id]).await
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn command(&self) -> Option<Vec<&str>> {
        let me = self.client.0.me.lock().unwrap();
        parse_command(&self.text, me.username.as_deref())
    }

    pub fn reply(&self, text: impl Into<String>) -> SendMessage<'_> {
        let send = self.client.send_message(self.chat.id, text);
        match self.chat.kind {
            ChatType::Private | ChatType::Bot => send,
            _ => send.reply_to(self.id),
        }
    }
}

impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Message")
            .field("id", &self.id)
            .field("chat", &self.chat)
            .field("from_user", &self.from_user)
            .field("sender_chat", &self.sender_chat)
            .field("date", &self.date)
            .field("text", &self.text)
            .field("outgoing", &self.outgoing)
            .field("edit_date", &self.edit_date)
            .finish_non_exhaustive()
    }
}

fn bare_message(id: i32, peer_id: base::Peer, date: i32) -> Result<types::Message> {
    let mut buf = Vec::new();
    0u32.serialize(&mut buf);
    0u32.serialize(&mut buf);
    id.serialize(&mut buf);
    peer_id.serialize(&mut buf);
    date.serialize(&mut buf);
    String::new().serialize(&mut buf);
    Ok(types::Message::deserialize_bare(&mut &buf[..])?)
}

pub(crate) fn parse_command<'a>(text: &'a str, my_username: Option<&str>) -> Option<Vec<&'a str>> {
    let body = text.strip_prefix('/')?;
    let (first, args) = body.split_once(char::is_whitespace).unwrap_or((body, ""));
    let name = match first.split_once('@') {
        Some((name, bot)) if my_username.is_some_and(|me| me.eq_ignore_ascii_case(bot)) => name,
        Some(_) => return None,
        None => first,
    };
    if name.is_empty() {
        return None;
    }
    Some(std::iter::once(name).chain(args.split_whitespace()).collect())
}

#[cfg(test)]
mod tests {
    use super::parse_command;

    #[test]
    fn parses_commands() {
        assert_eq!(parse_command("/start", None), Some(vec!["start"]));
        assert_eq!(parse_command("/echo  hi there", None), Some(vec!["echo", "hi", "there"]));
        assert_eq!(parse_command("/start@MyBot x", Some("mybot")), Some(vec!["start", "x"]));
        assert_eq!(parse_command("/start@OtherBot", Some("mybot")), None);
        assert_eq!(parse_command("/start@MyBot", None), None);
        assert_eq!(parse_command("start", None), None);
        assert_eq!(parse_command("/", None), None);
        assert_eq!(parse_command("/ start", None), None);
    }
}
