use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::ChatType;
use crate::errors::{Error, Result};
use crate::methods::{DownloadMedia, EditMessageText, SendMessage};
use crate::raw::{base, types};
use crate::types::{
    Chat, Command, ExternalReplyInfo, LinkPreviewOptions, Media, MessageEntity, MessageOrigin, ReplyMarkup,
    ServiceAction, TextQuote, User,
};
use crate::utils::get_peer;

#[derive(Debug, Clone)]
pub struct Message {
    pub id: i32,
    pub chat: Chat,
    pub from_user: Option<User>,
    pub sender_chat: Option<Chat>,
    pub date: i32,
    pub text: String,
    pub outgoing: bool,
    pub edit_date: Option<i32>,
    pub entities: Option<Vec<MessageEntity>>,
    pub caption: Option<String>,
    pub caption_entities: Option<Vec<MessageEntity>>,
    pub media: Option<Media>,
    pub link_preview_options: Option<LinkPreviewOptions>,
    pub reply_to_message_id: Option<i32>,
    pub message_thread_id: Option<i32>,
    pub is_topic_message: bool,
    pub quote: Option<TextQuote>,
    pub external_reply: Option<Box<ExternalReplyInfo>>,
    pub forward_origin: Option<MessageOrigin>,
    pub via_bot: Option<User>,
    pub media_group_id: Option<i64>,
    pub author_signature: Option<String>,
    pub has_protected_content: bool,
    pub service: Option<ServiceAction>,
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
        // The text of a media message is its caption; a link preview keeps it a text.
        let has_caption = !matches!(
            raw.media,
            None | Some(base::MessageMedia::MessageMediaEmpty | base::MessageMedia::MessageMediaWebPage(_))
        );
        let parsed_entities = raw.entities.as_deref().and_then(|e| MessageEntity::parse_all(e, entities));
        let (text, text_entities, caption, caption_entities) = if has_caption {
            (String::new(), None, (!raw.message.is_empty()).then(|| raw.message.clone()), parsed_entities)
        } else {
            (raw.message.clone(), parsed_entities, None, None)
        };
        let (reply_to_message_id, message_thread_id, is_topic_message, quote, external_reply) =
            reply_fields(&raw, entities);
        Message {
            id: raw.id,
            chat,
            from_user,
            sender_chat,
            date: raw.date,
            text,
            outgoing: raw.out,
            edit_date: raw.edit_date,
            entities: text_entities,
            caption,
            caption_entities,
            media: raw.media.as_ref().and_then(|m| Media::parse(m, entities)),
            link_preview_options: LinkPreviewOptions::parse(raw.media.as_ref(), raw.invert_media),
            reply_to_message_id,
            message_thread_id,
            is_topic_message,
            quote,
            external_reply,
            forward_origin: raw.fwd_from.as_ref().and_then(|h| MessageOrigin::parse(h, entities)),
            via_bot: raw.via_bot_id.map(|id| User::parse(id, entities)),
            media_group_id: raw.grouped_id,
            author_signature: raw.post_author.clone(),
            has_protected_content: raw.noforwards,
            service: None,
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
            base::Message::MessageService(m) => Message::parse_service(client, &m, entities),
            base::Message::MessageEmpty(_) => None,
        }
    }

    // `raw` holds a bare message with the service header fields, since only `Message` carries a sender and chat.
    fn parse_service(client: &Client, raw: &types::MessageService, entities: &Entities) -> Option<Message> {
        let service = ServiceAction::parse(&raw.action, entities)?;
        let mut bare = Message::bare(raw.id, raw.peer_id.clone(), raw.date);
        (bare.out, bare.post, bare.from_id) = (raw.out, raw.post, raw.from_id.clone());
        let mut message = Message::parse(client, Box::new(bare), entities);
        message.service = Some(service);
        Some(message)
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

    /// Fails at `.await` for media without a file (contact, location, venue, dice, poll).
    pub fn download(&self) -> DownloadMedia<'_> {
        DownloadMedia::new(&self.client, self.media.as_ref().and_then(Media::file_location).cloned())
    }

    pub fn command(&self) -> Option<Command<'_>> {
        self.command_with("/")
    }

    pub fn command_with(&self, prefixes: &str) -> Option<Command<'_>> {
        let command = Command::parse(self.caption.as_deref().unwrap_or(&self.text), prefixes)?;
        (command.bot.is_none() || command.is_for(self.client.0.me.lock().unwrap().username.as_deref()))
            .then_some(command)
    }

    pub fn reply(&self, text: impl Into<String>) -> SendMessage<'_> {
        let send = self.client.send_message(self.chat.id, text);
        match self.chat.kind {
            ChatType::Private | ChatType::Bot => send,
            _ => send.reply_to(self.id),
        }
    }
}

type ReplyFields = (Option<i32>, Option<i32>, bool, Option<TextQuote>, Option<Box<ExternalReplyInfo>>);

fn reply_fields(raw: &types::Message, entities: &Entities) -> ReplyFields {
    let Some(base::MessageReplyHeader::MessageReplyHeader(reply)) = &raw.reply_to else { return Default::default() };
    // In a forum, a header without reply_to_top_id points at the topic itself, not at a replied message.
    let (reply_to_message_id, message_thread_id) = if reply.forum_topic && reply.reply_to_top_id.is_none() {
        (None, reply.reply_to_msg_id)
    } else {
        (reply.reply_to_msg_id.filter(|_| reply.reply_to_peer_id.is_none()), reply.reply_to_top_id)
    };
    let quote = reply.quote_text.as_ref().map(|text| TextQuote {
        text: text.clone(),
        entities: reply.quote_entities.as_deref().and_then(|e| MessageEntity::parse_all(e, entities)),
        position: reply.quote_offset.unwrap_or(0),
        is_manual: reply.quote,
    });
    let external_reply = ExternalReplyInfo::parse(reply, entities).map(Box::new);
    (reply_to_message_id, message_thread_id, reply.forum_topic, quote, external_reply)
}

#[cfg(test)]
mod tests {
    use super::super::media::tests::document_media;
    use super::Message;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};
    use crate::types::{Community, Media, MessageOrigin, ServiceAction};
    use crate::utils::{get_channel_id, get_peer};

    fn header() -> types::MessageReplyHeader {
        types::MessageReplyHeader {
            reply_to_scheduled: false,
            forum_topic: false,
            quote: false,
            reply_to_ephemeral: false,
            reply_to_msg_id: None,
            reply_to_peer_id: None,
            reply_from: None,
            reply_media: None,
            reply_to_top_id: None,
            quote_text: None,
            quote_entities: None,
            quote_offset: None,
            todo_item_id: None,
            poll_option: None,
            quote_rich: None,
        }
    }

    fn forward(from_id: Option<base::Peer>, name: Option<&str>, post: Option<i32>) -> base::MessageFwdHeader {
        base::MessageFwdHeader::MessageFwdHeader(Box::new(types::MessageFwdHeader {
            imported: false,
            saved_out: false,
            from_id,
            from_name: name.map(Into::into),
            date: 100,
            channel_post: post,
            post_author: None,
            saved_from_peer: None,
            saved_from_msg_id: None,
            saved_from_id: None,
            saved_from_name: None,
            saved_date: None,
            psa_type: None,
        }))
    }

    fn parse(raw: &types::Message) -> Message {
        Message::parse(&Client::offline(), Box::new(raw.clone()), &Entities::default())
    }

    #[test]
    fn moves_media_text_to_the_caption() {
        let mut raw = Message::bare(1, get_peer(7), 0);
        raw.message = "/start now".into();
        let bold = types::MessageEntityBold { offset: 0, length: 6 };
        raw.entities = Some(vec![base::MessageEntity::MessageEntityBold(Box::new(bold))]);
        let text = parse(&raw);
        assert_eq!((text.text.as_str(), text.caption, text.entities.map(|e| e.len())), ("/start now", None, Some(1)));

        raw.media = Some(document_media("application/pdf", Vec::new()));
        let media = parse(&raw);
        assert_eq!((media.text.as_str(), media.caption.as_deref()), ("", Some("/start now")));
        let entities = (media.entities.as_ref().map(Vec::len), media.caption_entities.as_ref().map(Vec::len));
        assert_eq!((entities, media.document().is_some()), ((None, Some(1)), true));
        let command = media.command().map(|c| (c.name, c.args().collect::<Vec<_>>()));
        assert_eq!(command, Some(("start", vec!["now"])));
    }

    #[test]
    fn maps_replies_and_forwards() {
        let mut raw = Message::bare(1, get_peer(-1000000000005), 0);
        let topic = types::MessageReplyHeader { forum_topic: true, reply_to_msg_id: Some(10), ..header() };
        raw.reply_to = Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(topic)));
        let m = parse(&raw);
        assert_eq!((m.is_topic_message, m.message_thread_id, m.reply_to_message_id), (true, Some(10), None));

        let quoted = types::MessageReplyHeader {
            forum_topic: true,
            quote: true,
            reply_to_msg_id: Some(12),
            reply_to_top_id: Some(10),
            quote_text: Some("hi".into()),
            quote_offset: Some(3),
            ..header()
        };
        raw.reply_to = Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(quoted)));
        let m = parse(&raw);
        assert_eq!((m.message_thread_id, m.reply_to_message_id), (Some(10), Some(12)));
        let quote = m.quote.unwrap();
        assert_eq!((quote.text.as_str(), quote.position, quote.is_manual), ("hi", 3, true));

        let dice = types::MessageMediaDice { value: 6, emoticon: "🎲".into(), game_outcome: None };
        let external = types::MessageReplyHeader {
            reply_to_msg_id: Some(77),
            reply_to_peer_id: Some(get_peer(-1000000000003)),
            reply_from: Some(forward(Some(get_peer(-1000000000003)), None, Some(77))),
            reply_media: Some(base::MessageMedia::MessageMediaDice(Box::new(dice))),
            ..header()
        };
        raw.reply_to = Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(external)));
        raw.fwd_from = Some(forward(None, Some("Hidden"), None));
        let m = parse(&raw);
        assert_eq!(m.reply_to_message_id, None);
        let external = m.external_reply.unwrap();
        assert_eq!((external.chat.as_ref().map(|c| c.id), external.message_id), (Some(-1000000000003), Some(77)));
        assert!(matches!(external.origin, MessageOrigin::Channel { message_id: 77, .. }));
        assert!(matches!(&external.media, Some(Media::Dice(d)) if d.value == 6));
        let hidden = MessageOrigin::HiddenUser { date: 100, sender_user_name: "Hidden".into() };
        assert_eq!(m.forward_origin, Some(hidden));
    }

    #[test]
    fn maps_community_service_actions() {
        let client = Client::offline();
        let community = base::Chat::Community(Box::new(types::Community {
            creator: false,
            left: false,
            min: false,
            collapsed_in_dialogs: false,
            id: 7,
            access_hash: None,
            title: "Hub".into(),
            photo: base::ChatPhoto::ChatPhotoEmpty,
            date: 0,
            admin_rights: None,
            default_banned_rights: None,
        }));
        let entities = Entities::new(&client, Vec::new(), vec![community]);
        let service = |action| {
            let raw = types::MessageService {
                out: false,
                mentioned: false,
                media_unread: false,
                reactions_are_possible: false,
                silent: false,
                post: false,
                legacy: false,
                id: 1,
                from_id: None,
                peer_id: get_peer(get_channel_id(5)),
                saved_peer_id: None,
                reply_to: None,
                date: 1,
                action,
                reactions: None,
                ttl_period: None,
            };
            Message::from_raw(&client, base::Message::MessageService(Box::new(raw)), &entities)
        };
        let change = |community_id| {
            let action = types::MessageActionChangeCommunity { community_id };
            base::MessageAction::MessageActionChangeCommunity(Box::new(action))
        };
        let hub = Community { id: get_channel_id(7), name: "Hub".into() };

        let added = service(change(Some(7))).unwrap();
        assert_eq!(added.service, Some(ServiceAction::CommunityChatAdded(hub.clone())));
        assert_eq!(service(change(None)).unwrap().service, Some(ServiceAction::CommunityChatRemoved));
        let joined = types::MessageActionChatJoinedViaCommunity { community_id: 7 };
        let joined = service(base::MessageAction::MessageActionChatJoinedViaCommunity(Box::new(joined))).unwrap();
        assert_eq!(joined.service, Some(ServiceAction::CommunityChatJoined(hub)));
        assert_eq!(joined.chat.id, get_channel_id(5));
        assert!(service(base::MessageAction::MessageActionPinMessage).is_none());
    }
}
