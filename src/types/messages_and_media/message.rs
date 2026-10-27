use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::ChatType;
use crate::errors::{Error, Result};
use crate::methods::{DownloadMedia, EditMessageText, SendMessage};
use crate::raw::{base, types};
use crate::types::{Chat, Document, Photo, ReplyMarkup, User};
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
    pub photo: Option<Photo>,
    pub document: Option<Document>,
    pub raw: Box<types::Message>,
    client: Client,
}

impl Message {
    pub(crate) fn parse(client: &Client, raw: Box<types::Message>, entities: &Entities) -> Message {
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
        let (photo, document) = match &raw.media {
            Some(base::MessageMedia::MessageMediaPhoto(m)) => (m.photo.as_ref().and_then(Photo::parse), None),
            Some(base::MessageMedia::MessageMediaDocument(m)) => (None, m.document.as_ref().and_then(Document::parse)),
            _ => (None, None),
        };
        Message {
            id: raw.id,
            chat,
            from_user,
            sender_chat,
            date: raw.date,
            text: raw.message.clone(),
            outgoing: raw.out,
            edit_date: raw.edit_date,
            photo,
            document,
            raw,
            client: client.clone(),
        }
    }

    pub(crate) fn bare(id: i32, peer_id: base::Peer, date: i32) -> types::Message {
        types::Message {
            out: false,
            mentioned: false,
            media_unread: false,
            silent: false,
            post: false,
            from_scheduled: false,
            legacy: false,
            edit_hide: false,
            pinned: false,
            noforwards: false,
            invert_media: false,
            offline: false,
            video_processing_pending: false,
            paid_suggested_post_stars: false,
            paid_suggested_post_ton: false,
            id,
            from_id: None,
            from_boosts_applied: None,
            from_rank: None,
            peer_id,
            saved_peer_id: None,
            fwd_from: None,
            via_bot_id: None,
            via_business_bot_id: None,
            guestchat_via_from: None,
            reply_to: None,
            date,
            message: String::new(),
            media: None,
            reply_markup: None,
            entities: None,
            views: None,
            forwards: None,
            replies: None,
            edit_date: None,
            post_author: None,
            grouped_id: None,
            reactions: None,
            restriction_reason: None,
            ttl_period: None,
            quick_reply_shortcut_id: None,
            effect: None,
            factcheck: None,
            report_delivery_until_date: None,
            paid_message_stars: None,
            suggested_post: None,
            schedule_repeat_period: None,
            summary_from_language: None,
            rich_message: None,
        }
    }

    pub(crate) fn from_raw(client: &Client, raw: base::Message, entities: &Entities) -> Option<Message> {
        match raw {
            base::Message::Message(m) => Some(Message::parse(client, m, entities)),
            _ => None,
        }
    }

    pub(crate) fn unpack(client: &Client, messages: base::messages::Messages) -> (Vec<base::Message>, Entities) {
        let (messages, users, chats) = match messages {
            base::messages::Messages::Messages(m) => (m.messages, m.users, m.chats),
            base::messages::Messages::MessagesSlice(m) => (m.messages, m.users, m.chats),
            base::messages::Messages::ChannelMessages(m) => (m.messages, m.users, m.chats),
            base::messages::Messages::MessagesNotModified(_) => Default::default(),
        };
        (messages, Entities::new(client, users, chats))
    }

    pub(crate) fn from_updates(
        client: &Client,
        updates: base::Updates,
        chat_id: i64,
        sent: (String, Option<Vec<base::MessageEntity>>),
    ) -> Result<Message> {
        if let base::Updates::UpdateShortSentMessage(u) = updates {
            let raw = types::Message {
                out: u.out,
                from_id: u.out.then(|| get_peer(client.0.me.lock().unwrap().id)),
                message: sent.0,
                entities: u.entities.or(sent.1),
                media: u.media,
                ..Message::bare(u.id, get_peer(chat_id), u.date)
            };
            return Ok(Message::parse(client, Box::new(raw), &Entities::default()));
        }
        Message::all_from_updates(client, updates).next().ok_or(Error::Unexpected("Telegram sent no message back"))
    }

    pub(crate) fn all_from_updates(client: &Client, updates: base::Updates) -> impl Iterator<Item = Message> + '_ {
        let (updates, users, chats) = match updates {
            base::Updates::Updates(u) => (u.updates, u.users, u.chats),
            base::Updates::UpdatesCombined(u) => (u.updates, u.users, u.chats),
            base::Updates::UpdateShort(u) => (vec![u.update], Vec::new(), Vec::new()),
            _ => Default::default(),
        };
        let entities = Entities::new(client, users, chats);
        updates.into_iter().filter_map(move |u| {
            let message = match u {
                base::Update::UpdateNewMessage(u) => u.message,
                base::Update::UpdateNewChannelMessage(u) => u.message,
                base::Update::UpdateEditMessage(u) => u.message,
                base::Update::UpdateEditChannelMessage(u) => u.message,
                base::Update::UpdateBotNewBusinessMessage(u) => u.message,
                _ => return None,
            };
            Message::from_raw(client, message, &entities)
        })
    }

    pub fn reply_markup(&self) -> Option<ReplyMarkup> {
        ReplyMarkup::parse(self.raw.reply_markup.as_ref()?)
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

    /// Downloads the message's photo or document.
    pub fn download(&self) -> DownloadMedia<'_> {
        let location = self.photo.as_ref().map(|p| p.location.clone());
        DownloadMedia::new(&self.client, location.or_else(|| self.document.as_ref().map(|d| d.location.clone())))
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
            .field("photo", &self.photo)
            .field("document", &self.document)
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
