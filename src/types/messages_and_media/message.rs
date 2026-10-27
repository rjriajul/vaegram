use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::ChatType;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::{Chat, User};

#[derive(Clone)]
pub struct Message {
    pub id: i32,
    pub chat: Chat,
    pub from_user: Option<User>,
    pub sender_chat: Option<Chat>,
    pub date: i32,
    pub text: String,
    pub outgoing: bool,
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
            raw: Box::new(raw.clone()),
            client: client.clone(),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn command(&self) -> Option<Vec<&str>> {
        let me = self.client.0.me.lock().unwrap();
        parse_command(&self.text, me.username.as_deref())
    }

    pub async fn reply(&self, text: &str) -> Result<base::Updates> {
        let quote = !matches!(self.chat.kind, ChatType::Private | ChatType::Bot);
        self.client.send_text(self.chat.id, text, quote.then_some(self.id)).await
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
            .finish_non_exhaustive()
    }
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
