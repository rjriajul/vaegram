use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::ChatType;
use crate::errors::{Error, Result};
use crate::methods::{
    CopyMessage, DownloadMedia, EditMessageReplyMarkup, EditMessageText, ForwardMessages, OnBehalf, PinChatMessage,
    SendMessage, SendReaction, UnpinChatMessage,
};
use crate::raw::{base, types};
use crate::types::{
    Chat, Command, DirectMessagesTopic, ExternalReplyInfo, InlineKeyboardMarkup, LinkPreviewOptions, Media,
    MessageEntity, MessageOrigin, MessageText, PeerRef, ReactionCount, ReactionType, ReplyMarkup, RichMessage,
    ServiceAction, SuggestedPostInfo, TextQuote, User,
};
use crate::utils::get_peer;

#[derive(Debug, Clone)]
pub struct Message {
    pub id: i32,
    pub chat: Chat,
    pub from_user: Option<User>,
    pub sender_chat: Option<Chat>,
    pub date: i32,
    /// A rich message without plain text gets its blocks' text here, so text filters and commands still match.
    pub text: String,
    pub outgoing: bool,
    pub edit_date: Option<i32>,
    pub entities: Option<Vec<MessageEntity>>,
    pub caption: Option<String>,
    pub caption_entities: Option<Vec<MessageEntity>>,
    pub media: Option<Media>,
    pub rich_message: Option<RichMessage>,
    pub link_preview_options: Option<LinkPreviewOptions>,
    pub reply_to_message_id: Option<i32>,
    pub reply_to_checklist_task_id: Option<i32>,
    pub message_thread_id: Option<i32>,
    pub is_topic_message: bool,
    pub quote: Option<TextQuote>,
    pub external_reply: Option<Box<ExternalReplyInfo>>,
    pub forward_origin: Option<MessageOrigin>,
    pub via_bot: Option<User>,
    pub media_group_id: Option<i64>,
    pub author_signature: Option<String>,
    pub sender_boost_count: Option<i32>,
    pub has_protected_content: bool,
    pub service: Option<ServiceAction>,
    pub business_connection_id: Option<String>,
    pub direct_messages_topic: Option<DirectMessagesTopic>,
    pub reactions: Vec<ReactionCount>,
    /// The raw message, minus `rich_message`, which moves to [`Message::rich_message`].
    pub raw: Box<types::Message>,
    client: Client,
}

impl Message {
    pub(crate) fn parse(client: &Client, raw: Box<types::Message>, entities: &Entities) -> Message {
        Message::build(client, raw, None, entities)
    }

    fn build(
        client: &Client,
        mut raw: Box<types::Message>,
        service: Option<ServiceAction>,
        entities: &Entities,
    ) -> Message {
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
        let (
            reply_to_message_id,
            reply_to_checklist_task_id,
            message_thread_id,
            is_topic_message,
            quote,
            external_reply,
        ) = reply_fields(&raw, service.as_ref(), entities);
        let direct_messages_topic = match &raw.saved_peer_id {
            Some(peer) if chat.is_direct_messages => Some(DirectMessagesTopic::parse(peer, entities)),
            _ => None,
        };
        let rich_message = raw.rich_message.take().map(RichMessage::parse);
        let text = match &rich_message {
            Some(rich) if text.is_empty() && caption.is_none() => rich.text(),
            _ => text,
        };
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
            rich_message,
            link_preview_options: LinkPreviewOptions::parse(raw.media.as_ref(), raw.invert_media),
            reply_to_message_id,
            reply_to_checklist_task_id,
            message_thread_id,
            is_topic_message,
            quote,
            external_reply,
            forward_origin: raw.fwd_from.as_ref().and_then(|h| MessageOrigin::parse(h, entities)),
            via_bot: raw.via_bot_id.map(|id| User::parse(id, entities)),
            media_group_id: raw.grouped_id,
            author_signature: raw.post_author.clone(),
            sender_boost_count: raw.from_boosts_applied,
            has_protected_content: raw.noforwards,
            service,
            business_connection_id: None,
            direct_messages_topic,
            reactions: raw.reactions.as_ref().map_or(Vec::new(), |base::MessageReactions::MessageReactions(r)| {
                r.results.iter().filter_map(ReactionCount::parse).collect()
            }),
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
        let service = ServiceAction::parse(raw, entities)?;
        let mut bare = Message::bare(raw.id, raw.peer_id.clone(), raw.date);
        (bare.out, bare.post, bare.from_id) = (raw.out, raw.post, raw.from_id.clone());
        bare.saved_peer_id = raw.saved_peer_id.clone();
        // A pin's header points at the pinned message, which the service message doesn't reply to.
        if !matches!(raw.action, base::MessageAction::MessageActionPinMessage) {
            bare.reply_to = raw.reply_to.clone();
        }
        Some(Message::build(client, Box::new(bare), Some(service), entities))
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
                base::Update::UpdateBotEditBusinessMessage(u) => u.message,
                _ => return None,
            };
            Message::from_raw(client, message, &entities)
        })
    }

    pub fn reply_markup(&self) -> Option<ReplyMarkup> {
        ReplyMarkup::parse(self.raw.reply_markup.as_ref()?)
    }

    /// A paid suggested post; editing it within 24 hours of publishing refunds the payment.
    pub fn is_paid_post(&self) -> bool {
        self.raw.paid_suggested_post_stars || self.raw.paid_suggested_post_ton
    }

    pub fn suggested_post_info(&self) -> Option<SuggestedPostInfo> {
        self.raw.suggested_post.as_ref().map(SuggestedPostInfo::parse)
    }

    /// Every request built from a business message has to go through the connection it arrived on.
    fn on_behalf<B: OnBehalf>(&self, mut request: B) -> B {
        request.business_connection().clone_from(&self.business_connection_id);
        request
    }

    pub fn edit_text(&self, text: impl Into<MessageText>) -> EditMessageText<'_> {
        self.on_behalf(self.client.edit_message_text(self.chat.id, self.id, text))
    }

    pub fn edit_caption(&self, caption: impl Into<String>) -> EditMessageText<'_> {
        self.edit_text(caption.into())
    }

    /// `None` removes the keyboard.
    pub fn edit_reply_markup(&self, reply_markup: Option<InlineKeyboardMarkup>) -> EditMessageReplyMarkup<'_> {
        self.on_behalf(self.client.edit_message_reply_markup(self.chat.id, self.id, reply_markup))
    }

    pub async fn delete(&self) -> Result<()> {
        match &self.business_connection_id {
            Some(id) => self.client.delete_business_messages(id, &[self.id]).await,
            None => self.client.delete_messages(self.chat.id, &[self.id]).await,
        }
    }

    pub fn forward(&self, chat_id: impl Into<PeerRef>) -> ForwardMessages<'_> {
        self.client.forward_messages(chat_id, self.chat.id, std::slice::from_ref(&self.id))
    }

    pub fn copy(&self, chat_id: impl Into<PeerRef>) -> CopyMessage<'_> {
        self.client.copy_message(chat_id, self.chat.id, self.id)
    }

    pub fn pin(&self) -> PinChatMessage<'_> {
        self.on_behalf(self.client.pin_chat_message(self.chat.id, self.id))
    }

    pub fn unpin(&self) -> UnpinChatMessage<'_> {
        self.on_behalf(self.client.unpin_chat_message(self.chat.id, self.id))
    }

    // Business bot rights don't cover reactions, so the bot reacts as itself.
    pub fn react(&self, reaction: impl Into<ReactionType>) -> SendReaction<'_> {
        self.client.send_reaction(self.chat.id, self.id).reaction(reaction)
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
        self.answer(text).reply_to(self.id)
    }

    /// Sends `text` to this chat without a quote, in the same topic and through the same business connection.
    pub fn answer(&self, text: impl Into<String>) -> SendMessage<'_> {
        let mut send = self.on_behalf(self.client.send_message(self.chat.id, text));
        if let Some(topic) = &self.direct_messages_topic {
            send = send.direct_messages_topic_id(topic.topic_id);
        }
        match self.message_thread_id {
            Some(thread) if self.is_topic_message => send.message_thread_id(thread),
            _ => send,
        }
    }
}

type ReplyFields = (Option<i32>, Option<i32>, Option<i32>, bool, Option<TextQuote>, Option<Box<ExternalReplyInfo>>);

fn reply_fields(raw: &types::Message, service: Option<&ServiceAction>, entities: &Entities) -> ReplyFields {
    // The message that creates a topic is its root, so the topic id is the message's own id.
    if matches!(service, Some(ServiceAction::ForumTopicCreated(_))) {
        return (None, None, Some(raw.id), true, None, None);
    }
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
    (reply_to_message_id, reply.todo_item_id, message_thread_id, reply.forum_topic, quote, external_reply)
}

#[cfg(test)]
mod tests {
    use super::super::media::tests::document_media;
    use super::Message;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};
    use crate::types::{Community, ForumTopicCreated, ForumTopicEdited, Media, MessageOrigin, ServiceAction};
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
    fn maps_service_actions() {
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
        let created = types::MessageActionManagedBotCreated { bot_id: 42 };
        let created = service(base::MessageAction::MessageActionManagedBotCreated(Box::new(created))).unwrap();
        assert!(matches!(created.service, Some(ServiceAction::ManagedBotCreated(bot)) if bot.id == 42));
        assert!(service(base::MessageAction::MessageActionPinMessage).is_none());
    }

    #[test]
    fn maps_forum_topic_service_actions() {
        let client = Client::offline();
        let service = |reply_to: Option<types::MessageReplyHeader>, action| {
            let raw = types::MessageService {
                out: false,
                mentioned: false,
                media_unread: false,
                reactions_are_possible: false,
                silent: false,
                post: false,
                legacy: false,
                id: 9,
                from_id: None,
                peer_id: get_peer(get_channel_id(5)),
                saved_peer_id: None,
                reply_to: reply_to.map(|r| base::MessageReplyHeader::MessageReplyHeader(Box::new(r))),
                date: 1,
                action,
                reactions: None,
                ttl_period: None,
            };
            Message::from_raw(&client, base::Message::MessageService(Box::new(raw)), &Entities::default()).unwrap()
        };
        let edit = |title: Option<&str>, closed, hidden| {
            let edit =
                types::MessageActionTopicEdit { title: title.map(Into::into), icon_emoji_id: None, closed, hidden };
            base::MessageAction::MessageActionTopicEdit(Box::new(edit))
        };
        let in_topic = Some(types::MessageReplyHeader { forum_topic: true, reply_to_msg_id: Some(4), ..header() });

        let create = types::MessageActionTopicCreate {
            title_missing: true,
            title: String::new(),
            icon_color: 0x6FB9F0,
            icon_emoji_id: None,
        };
        let created = service(None, base::MessageAction::MessageActionTopicCreate(Box::new(create)));
        let topic = ForumTopicCreated {
            title: String::new(),
            icon_color: 0x6FB9F0,
            icon_emoji_id: None,
            is_name_implicit: true,
        };
        assert_eq!(created.service, Some(ServiceAction::ForumTopicCreated(topic)));
        assert_eq!((created.message_thread_id, created.is_topic_message), (Some(9), true));

        let renamed = service(in_topic.clone(), edit(Some("New"), None, None));
        let edited = ForumTopicEdited { title: Some("New".into()), icon_emoji_id: None };
        assert_eq!(
            (renamed.service, renamed.message_thread_id),
            (Some(ServiceAction::ForumTopicEdited(edited)), Some(4))
        );
        assert_eq!(renamed.reply_to_message_id, None);
        assert_eq!(service(in_topic, edit(None, Some(true), None)).service, Some(ServiceAction::ForumTopicClosed));
        assert_eq!(service(None, edit(None, Some(false), None)).service, Some(ServiceAction::ForumTopicReopened));
        assert_eq!(service(None, edit(None, None, Some(true))).service, Some(ServiceAction::GeneralForumTopicHidden));
        assert_eq!(
            service(None, edit(None, None, Some(false))).service,
            Some(ServiceAction::GeneralForumTopicUnhidden)
        );
    }

    #[test]
    fn rich_message_moves_out_of_raw_and_fills_text() {
        let mut raw = Message::bare(1, get_peer(7), 0);
        let plain = |text: &str| base::RichText::TextPlain(Box::new(types::TextPlain { text: text.into() }));
        let link = types::TextUrl { text: plain("Bob"), url: "https://t.me".into(), webpage_id: 0 };
        let bold =
            base::RichText::TextBold(Box::new(types::TextBold { text: base::RichText::TextUrl(Box::new(link)) }));
        let texts = vec![plain("hi "), bold, base::RichText::TextEmpty];
        let blocks = vec![
            base::PageBlock::PageBlockHeading1(Box::new(types::PageBlockHeading1 { text: plain("/start") })),
            base::PageBlock::PageBlockDivider,
            base::PageBlock::PageBlockParagraph(Box::new(types::PageBlockParagraph {
                text: base::RichText::TextConcat(Box::new(types::TextConcat { texts })),
            })),
        ];
        let rich = types::RichMessage { rtl: true, part: false, blocks, photos: vec![], documents: vec![] };
        raw.rich_message = Some(base::RichMessage::RichMessage(Box::new(rich)));
        let message = parse(&raw);
        assert_eq!(message.text, "/start\nhi Bob");
        assert_eq!(message.command().map(|c| c.name), Some("start"));
        assert!(message.rich_message.is_some_and(|r| r.rtl));
        assert_eq!(message.raw.rich_message, None);
    }

    #[tokio::test]
    async fn business_messages_are_answered_through_their_connection() {
        let (client, mut rx) = Client::recording();
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        let raw = Message::bare(5, get_peer(7), 0);
        let mut message = Message::parse(&client, Box::new(raw), &Entities::default());
        message.business_connection_id = Some("c".into());
        assert!(message.delete().await.is_err());
        assert_eq!(rx.try_recv().expect("no request sent")[..4], 0xdd289f8e_u32.to_le_bytes());
        let sent = [
            message.reply("hi").await.map(drop),
            message.edit_text("hi").await.map(drop),
            message.pin().await,
            message.unpin().await,
        ];
        for sent in sent {
            assert!(sent.is_err());
            assert_eq!(rx.try_recv().expect("no request sent")[..4], 0xdd289f8e_u32.to_le_bytes());
        }
    }

    #[tokio::test]
    async fn reply_quotes_and_answer_stays_in_the_topic() {
        let (client, mut rx) = Client::recording();
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        let raw = Message::bare(5, get_peer(7), 0);
        let mut message = Message::parse(&client, Box::new(raw), &Entities::default());
        let _ = message.reply("hi").await;
        let _ = message.answer("hi").await;
        (message.message_thread_id, message.is_topic_message) = (Some(3), true);
        let _ = message.answer("hi").await;
        // reply_to is flags.0 of messages.sendMessage, right after the constructor id.
        let mut has_reply_to = || rx.try_recv().expect("no request sent")[4] & 1 == 1;
        assert_eq!((has_reply_to(), has_reply_to(), has_reply_to()), (true, false, true));
    }
}
