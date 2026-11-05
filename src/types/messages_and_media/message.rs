use std::sync::Arc;

use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::{ChatJoinType, ChatType};
use crate::errors::{Error, Result};
use crate::methods::{
    CopyMessage, DownloadMedia, DownloadSource, EditMessageReplyMarkup, EditMessageText, ForwardMessages, OnBehalf,
    PinChatMessage, SendMediaGroup, SendMessage, SendReaction, UnpinChatMessage,
};
use crate::raw::{base, types};
use crate::types::push_plain;
use crate::types::{
    Chat, Command, DirectMessagesTopic, ExternalReplyInfo, FactCheck, ForumTopic, InlineKeyboardMarkup, InputMedia,
    LinkPreviewOptions, Media, MessageEntity, MessageOrigin, MessageReactions, MessageText, PeerRef, ReactionType,
    ReplyMarkup, RestrictionReason, RichMessage, RichText, ServiceAction, Story, SuggestedPostInfo, Text, TextQuote,
    User, WebPage,
};
use crate::utils::{get_peer, get_peer_id};

/// A message in a chat, with its sender, content and metadata.
///
/// Media messages keep their text in `caption` and leave `text` empty. A link preview doesn't count as media.
/// The methods on this type, such as
/// [`reply`](Message::reply) and [`edit_text`](Message::edit_text), act on this message in its own chat.
#[derive(Debug, Clone)]
pub struct Message {
    /// Id of the message, unique within `chat`.
    pub id: i32,
    /// The chat the message belongs to.
    pub chat: Chat,
    /// The sending user. `None` for channel posts and for messages sent on behalf of a chat.
    pub from_user: Option<User>,
    /// The chat the message was sent on behalf of. `None` when a user sent it as themselves.
    pub sender_chat: Option<Chat>,
    /// When the message was sent, as a Unix timestamp.
    pub date: i32,
    /// A rich message without plain text gets its blocks' text here, so text filters and commands still match.
    pub text: String,
    /// The message was sent by the account this client is logged in as.
    pub outgoing: bool,
    /// When the message was last edited, as a Unix timestamp. `None` if it never was.
    pub edit_date: Option<i32>,
    /// Formatting entities of `text`.
    pub entities: Option<Vec<MessageEntity>>,
    /// The text of a media message. `None` for a text message, or if the sender wrote no caption.
    pub caption: Option<String>,
    /// Formatting entities of `caption`.
    pub caption_entities: Option<Vec<MessageEntity>>,
    /// The attached media. `None` for a text message and for media of a kind [`Media`] doesn't cover.
    pub media: Option<Media>,
    /// The link preview of a text message; `media` stays `None` and the text stays in `text`.
    pub web_page: Option<WebPage>,
    /// The structured content of a rich message. `None` for other messages.
    pub rich_message: Option<RichMessage>,
    /// How the link preview is shown. `None` if the message has no link preview.
    pub link_preview_options: Option<LinkPreviewOptions>,
    /// Id of the message this one replies to. `None` if it's not a reply in this chat.
    pub reply_to_message_id: Option<i32>,
    /// The checklist task this message replies to.
    pub reply_to_checklist_task_id: Option<i32>,
    /// The story this message replies to.
    pub reply_to_story_id: Option<i32>,
    /// The chat that posted the replied story: a user, or a `-100…` channel id.
    pub reply_to_story_user_id: Option<i64>,
    /// The poll option this message replies to, the same bytes as [`PollOption::data`](crate::types::PollOption).
    pub reply_to_poll_option_id: Option<Vec<u8>>,
    /// Id of the forum topic or reply thread the message is in. `None` outside of one, and outside forums for a
    /// direct reply to the thread's first message.
    pub message_thread_id: Option<i32>,
    /// The message is in a forum topic.
    pub is_topic_message: bool,
    /// The part of the replied message that this one quotes. `None` if the reply quotes nothing.
    pub quote: Option<TextQuote>,
    /// Details of a reply to a message in another chat. `None` for other messages.
    pub external_reply: Option<Box<ExternalReplyInfo>>,
    /// Where a forwarded message came from. `None` if it isn't a forward.
    pub forward_origin: Option<MessageOrigin>,
    /// The message is a channel post that Telegram copied into the channel's linked discussion group.
    pub is_automatic_forward: bool,
    /// The inline bot the message was sent through.
    pub via_bot: Option<User>,
    /// Shared by the messages of one album. `None` if the message isn't part of an album.
    pub media_group_id: Option<i64>,
    /// Signature of the post's author, in channels that sign posts.
    pub author_signature: Option<String>,
    /// How many boosts the sender gave the group.
    pub sender_boost_count: Option<i32>,
    /// The message can't be forwarded or saved.
    pub has_protected_content: bool,
    /// What happened, for a service message. `None` for an ordinary message.
    pub service: Option<ServiceAction>,
    /// Id of the business connection the message came through. `None` outside of business chats.
    pub business_connection_id: Option<String>,
    /// The direct messages topic the message is in. `None` outside of a channel's direct messages.
    pub direct_messages_topic: Option<DirectMessagesTopic>,
    /// `None` when nobody has reacted, or the message comes from a request that leaves reactions out.
    pub reactions: Option<MessageReactions>,
    /// The account was mentioned in it, or it replies to the account's message.
    pub mentioned: bool,
    /// Its voice or video note, or self-destructing media, hasn't been played yet.
    pub unread_media: bool,
    /// Sent without a notification sound.
    pub silent: bool,
    /// The message is a post in a channel.
    pub channel_post: bool,
    /// Sent by the server at a time the sender had scheduled.
    pub from_scheduled: bool,
    /// The edit is hidden from the chat, so the message shows no "edited" mark.
    pub edit_hidden: bool,
    /// The message is pinned.
    pub pinned: bool,
    /// The caption is shown above the media instead of below it.
    pub show_caption_above_media: bool,
    /// Sent by a business bot or an away message while the account was offline.
    pub from_offline: bool,
    /// A video still being processed; others can't see the message yet.
    pub video_processing_pending: bool,
    /// The business bot that sent it on behalf of the account.
    pub sender_business_bot: Option<User>,
    /// The sender's custom tag (rank) in the group.
    pub sender_tag: Option<String>,
    /// The user whose guest bot request produced this message.
    pub guest_bot_caller_user: Option<User>,
    /// The chat whose guest bot request produced this message.
    pub guest_bot_caller_chat: Option<Chat>,
    /// How many times the message was viewed. `None` if Telegram doesn't count views for it.
    pub views: Option<i32>,
    /// How many times the message was forwarded. `None` if Telegram doesn't count forwards for it.
    pub forwards: Option<i32>,
    /// The message effect shown when it arrives.
    pub effect_id: Option<i64>,
    /// Stars the sender paid to send it.
    pub send_paid_messages_stars: Option<i64>,
    /// For a scheduled message, how often in seconds it repeats.
    pub repeat_period: Option<i32>,
    /// The language a summary of the message was made from.
    pub summary_language_code: Option<String>,
    /// Waiting to be sent at a scheduled date: it came from a scheduling answer or `get_scheduled_messages`.
    pub scheduled: bool,
    /// For a service message about someone joining: how they came in.
    pub chat_join_type: Option<ChatJoinType>,
    /// Why the message is hidden on some platforms; empty when it isn't.
    pub restriction_reason: Vec<RestrictionReason>,
    /// The fact check attached to the message, if any.
    pub fact_check: Option<FactCheck>,
    /// The raw message, minus `rich_message`, which moves to [`Message::rich_message`] and leaves its text in
    /// `message`.
    pub(crate) raw: Box<types::Message>,
    // The replied message a business update carries, since the bot can't read the account's history.
    replied: Option<Box<Message>>,
    // What the client's fetch options looked up before handlers saw the message.
    pub(crate) prefetched: Option<Box<Prefetched>>,
    client: Client,
}

/// Each slot is `None` when nothing was fetched (the option is off, or the fetch failed), and `Some(None)` when
/// Telegram answered that there's nothing there.
#[derive(Debug, Clone, Default)]
pub(crate) struct Prefetched {
    pub(crate) reply: Option<Option<Message>>,
    pub(crate) pinned: Option<Option<Message>>,
    pub(crate) topic: Option<Option<ForumTopic>>,
    pub(crate) story: Option<Option<Story>>,
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
        Message::fill_rich_text(&mut raw);
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
        let has_caption = takes_caption(raw.media.as_ref());
        let parsed_entities = raw.entities.as_deref().and_then(|e| MessageEntity::parse_all(e, entities));
        let (text, text_entities, caption, caption_entities) = if has_caption {
            (String::new(), None, (!raw.message.is_empty()).then(|| raw.message.clone()), parsed_entities)
        } else {
            (raw.message.clone(), parsed_entities, None, None)
        };
        let ReplyFields {
            reply_to_message_id,
            reply_to_checklist_task_id,
            reply_to_story_id,
            reply_to_story_user_id,
            reply_to_poll_option_id,
            message_thread_id,
            is_topic_message,
            quote,
            external_reply,
        } = reply_fields(client, &raw, service.as_ref(), entities);
        let direct_messages_topic = match &raw.saved_peer_id {
            Some(peer) if chat.is_direct_messages => Some(DirectMessagesTopic::parse(peer, entities)),
            _ => None,
        };
        let rich_message = raw.rich_message.take().map(RichMessage::parse);
        let is_automatic_forward = is_automatic_forward(&chat, raw.fwd_from.as_ref());
        let (guest_bot_caller_user, guest_bot_caller_chat) = match &raw.guestchat_via_from {
            Some(base::Peer::PeerUser(p)) => (Some(User::parse(p.user_id, entities)), None),
            Some(peer) => (None, Some(Chat::parse(peer, entities, false))),
            None => (None, None),
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
            media: raw.media.as_ref().and_then(|m| Media::parse(client, m, entities)),
            web_page: raw.media.as_ref().and_then(WebPage::parse),
            rich_message,
            link_preview_options: LinkPreviewOptions::parse(raw.media.as_ref(), raw.invert_media),
            reply_to_message_id,
            reply_to_checklist_task_id,
            reply_to_story_id,
            reply_to_story_user_id,
            reply_to_poll_option_id,
            message_thread_id,
            is_topic_message,
            quote,
            external_reply,
            forward_origin: raw.fwd_from.as_ref().and_then(|h| MessageOrigin::parse(h, entities)),
            is_automatic_forward,
            via_bot: raw.via_bot_id.map(|id| User::parse(id, entities)),
            media_group_id: raw.grouped_id,
            author_signature: raw.post_author.clone(),
            sender_boost_count: raw.from_boosts_applied,
            has_protected_content: raw.noforwards,
            service,
            business_connection_id: None,
            direct_messages_topic,
            reactions: raw.reactions.as_ref().map(|r| MessageReactions::parse(r, entities)),
            mentioned: raw.mentioned,
            unread_media: raw.media_unread,
            silent: raw.silent,
            channel_post: raw.post,
            from_scheduled: raw.from_scheduled,
            edit_hidden: raw.edit_hide,
            pinned: raw.pinned,
            show_caption_above_media: raw.invert_media,
            from_offline: raw.offline,
            video_processing_pending: raw.video_processing_pending,
            sender_business_bot: raw.via_business_bot_id.map(|id| User::parse(id, entities)),
            sender_tag: raw.from_rank.clone(),
            guest_bot_caller_user,
            guest_bot_caller_chat,
            views: raw.views,
            forwards: raw.forwards,
            effect_id: raw.effect,
            send_paid_messages_stars: raw.paid_message_stars,
            repeat_period: raw.schedule_repeat_period,
            summary_language_code: raw.summary_from_language.clone(),
            scheduled: false,
            chat_join_type: None,
            restriction_reason: raw
                .restriction_reason
                .as_deref()
                .map_or(Vec::new(), |r| r.iter().map(RestrictionReason::parse).collect()),
            fact_check: raw.factcheck.as_ref().map(|f| FactCheck::parse(f, entities)),
            raw,
            replied: None,
            prefetched: None,
            client: client.clone(),
        }
    }

    /// Filters read `message`, so a rich message without plain text carries its blocks' text there. Not on media:
    /// there `message` is the caption the sender set, and the blocks stay in `rich_message` only.
    pub(crate) fn fill_rich_text(raw: &mut types::Message) {
        if !raw.message.is_empty() || takes_caption(raw.media.as_ref()) {
            return;
        }
        if let Some(base::RichMessage::RichMessage(rich)) = &raw.rich_message {
            raw.message = RichMessage::plain_text(rich);
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
        (bare.out, bare.mentioned, bare.post, bare.from_id) = (raw.out, raw.mentioned, raw.post, raw.from_id.clone());
        (bare.silent, bare.media_unread) = (raw.silent, raw.media_unread);
        (bare.saved_peer_id, bare.reactions) = (raw.saved_peer_id.clone(), raw.reactions.clone());
        // A pin's header points at the pinned message, which the service message doesn't reply to. In a forum it
        // still names the topic, so that part stays as the header of a message posted in the topic.
        let pin = matches!(raw.action, base::MessageAction::MessageActionPinMessage);
        bare.reply_to = match &raw.reply_to {
            Some(base::MessageReplyHeader::MessageReplyHeader(h)) if pin && h.forum_topic => {
                let thread = h.reply_to_top_id.or(h.reply_to_msg_id);
                let topic =
                    types::MessageReplyHeader { reply_to_msg_id: thread, reply_to_top_id: None, ..(**h).clone() };
                Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(topic)))
            }
            _ if pin => None,
            reply_to => reply_to.clone(),
        };
        let chat_join_type = match raw.action {
            base::MessageAction::MessageActionChatAddUser(_) => Some(ChatJoinType::ByAdd),
            base::MessageAction::MessageActionChatJoinedByLink(_) => Some(ChatJoinType::ByLink),
            base::MessageAction::MessageActionChatJoinedByRequest => Some(ChatJoinType::ByRequest),
            _ => None,
        };
        let mut message = Message::build(client, Box::new(bare), Some(service), entities);
        message.chat_join_type = chat_join_type;
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
            // Own sends never come back as updates, so this is how a button on the bot's own message gets cached.
            if client.0.caches.caches_messages() {
                client.0.caches.store(&base::Message::Message(Box::new(raw.clone())), &Arc::default());
            }
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
        let entities = Arc::new(Entities::new(client, users, chats));
        updates.into_iter().filter_map(move |u| {
            let scheduled = matches!(u, base::Update::UpdateNewScheduledMessage(_));
            let (message, business) = match u {
                base::Update::UpdateNewMessage(u) => (u.message, None),
                base::Update::UpdateNewChannelMessage(u) => (u.message, None),
                base::Update::UpdateEditMessage(u) => (u.message, None),
                base::Update::UpdateEditChannelMessage(u) => (u.message, None),
                base::Update::UpdateBotNewBusinessMessage(u) => {
                    (u.message, Some((u.connection_id, u.reply_to_message)))
                }
                base::Update::UpdateBotEditBusinessMessage(u) => {
                    (u.message, Some((u.connection_id, u.reply_to_message)))
                }
                // Scheduling a message, or editing one that is still scheduled, answers with this.
                base::Update::UpdateNewScheduledMessage(u) => (u.message, None),
                _ => return None,
            };
            if !scheduled && business.is_none() {
                client.0.caches.store(&message, &entities);
            }
            let mut message = Message::from_raw(client, message, &entities)?;
            message.scheduled = scheduled;
            if let Some((connection_id, reply_to)) = business {
                message.set_business(client, connection_id, reply_to, &entities);
            }
            Some(message)
        })
    }

    pub(crate) fn set_business(
        &mut self,
        client: &Client,
        connection_id: String,
        reply_to: Option<base::Message>,
        entities: &Entities,
    ) {
        let replied = reply_to.and_then(|m| Message::from_raw(client, m, entities));
        self.replied = replied.map(|mut m| {
            m.business_connection_id = Some(connection_id.clone());
            Box::new(m)
        });
        self.business_connection_id = Some(connection_id);
    }

    /// The reply markup attached to the message. `None` if it has none.
    pub fn reply_markup(&self) -> Option<ReplyMarkup> {
        ReplyMarkup::parse(self.raw.reply_markup.as_ref()?)
    }

    /// A paid suggested post; editing it within 24 hours of publishing refunds the payment.
    pub fn is_paid_post(&self) -> bool {
        self.raw.paid_suggested_post_stars || self.raw.paid_suggested_post_ton
    }

    /// Details of the suggested post this message is. `None` for other messages.
    pub fn suggested_post_info(&self) -> Option<SuggestedPostInfo> {
        self.raw.suggested_post.as_ref().map(SuggestedPostInfo::parse)
    }

    /// Every request built from a business message has to go through the connection it arrived on.
    pub(super) fn on_behalf<B: OnBehalf>(&self, mut request: B) -> B {
        request.business_connection().clone_from(&self.business_connection_id);
        request
    }

    /// Edits the text of this message.
    ///
    /// Awaiting the request returns the edited [`Message`]. A message that came through a business connection is
    /// edited through that connection.
    ///
    /// ```no_run
    /// # async fn f(message: &vaegram::types::Message) -> vaegram::Result<()> {
    /// let edited = message.edit_text("Updated").await?;
    /// # Ok(()) }
    /// ```
    pub fn edit_text(&self, text: impl Into<MessageText>) -> EditMessageText<'_> {
        self.on_behalf(self.client.edit_message_text(self.chat.id, self.id, text))
    }

    /// Edits the caption of this media message.
    ///
    /// Telegram stores a caption as the message text, so this is the same request as
    /// [`edit_text`](Message::edit_text).
    pub fn edit_caption(&self, caption: impl Into<String>) -> EditMessageText<'_> {
        self.edit_text(caption.into())
    }

    /// Replaces the inline keyboard of this message. `None` removes the keyboard.
    pub fn edit_reply_markup(&self, reply_markup: Option<InlineKeyboardMarkup>) -> EditMessageReplyMarkup<'_> {
        self.on_behalf(self.client.edit_message_reply_markup(self.chat.id, self.id, reply_markup))
    }

    /// Deletes this message.
    ///
    /// A message that arrived through a business connection is deleted through that connection.
    ///
    /// ```no_run
    /// # async fn f(message: &vaegram::types::Message) -> vaegram::Result<()> {
    /// message.delete().await?;
    /// # Ok(()) }
    /// ```
    pub async fn delete(&self) -> Result<()> {
        match &self.business_connection_id {
            Some(id) => self.client.delete_business_messages(id, &[self.id]).await,
            None => self.client.delete_messages(self.chat.id, &[self.id]).await,
        }
    }

    /// Forwards this message to another chat.
    ///
    /// Awaiting the request returns the forwarded messages as a `Vec` of [`Message`].
    ///
    /// ```no_run
    /// # async fn f(message: &vaegram::types::Message) -> vaegram::Result<()> {
    /// let forwarded = message.forward("me").await?;
    /// # Ok(()) }
    /// ```
    pub fn forward(&self, chat_id: impl Into<PeerRef>) -> ForwardMessages<'_> {
        self.client.forward_messages(chat_id, self.chat.id, std::slice::from_ref(&self.id))
    }

    /// Copies this message to another chat, without a link to the original.
    ///
    /// Awaiting the request returns the new [`Message`].
    ///
    /// ```no_run
    /// # async fn f(message: &vaegram::types::Message) -> vaegram::Result<()> {
    /// let copy = message.copy("me").await?;
    /// # Ok(()) }
    /// ```
    pub fn copy(&self, chat_id: impl Into<PeerRef>) -> CopyMessage<'_> {
        self.client.copy_message(chat_id, self.chat.id, self.id)
    }

    /// Pins this message in its chat.
    pub fn pin(&self) -> PinChatMessage<'_> {
        self.on_behalf(self.client.pin_chat_message(self.chat.id, self.id))
    }

    /// Unpins this message in its chat.
    pub fn unpin(&self) -> UnpinChatMessage<'_> {
        self.on_behalf(self.client.unpin_chat_message(self.chat.id, self.id))
    }

    // Business bot rights don't cover reactions, so the bot reacts as itself.
    /// Adds a reaction to this message. On a business message the bot reacts as itself, not as the business account.
    pub fn react(&self, reaction: impl Into<ReactionType>) -> SendReaction<'_> {
        self.client.send_reaction(self.chat.id, self.id).reaction(reaction)
    }

    /// The [`Client`] that received this message.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Downloads the file of this message's media.
    ///
    /// Awaiting the request returns the file as bytes. It fails with `Error::InvalidArgument` unless the media is a
    /// photo, live photo, document, animation, audio, video, video note, voice note or sticker. The returned
    /// [`DownloadMedia`] has the same setters as [`Client::download_media`].
    ///
    /// ```no_run
    /// # async fn f(message: &vaegram::types::Message) -> vaegram::Result<()> {
    /// let bytes = message.download().await?;
    /// # Ok(()) }
    /// ```
    pub fn download(&self) -> DownloadMedia<'_> {
        let location = self.media.as_ref().and_then(Media::file_location).cloned();
        let location = location.ok_or(Error::InvalidArgument("this message has no photo or document to download"));
        // A business chat's message ids aren't the bot's, and they can't be re-read through the connection, so an
        // expired reference there isn't renewed.
        let source = self.business_connection_id.is_none().then_some(DownloadSource::Message(self.chat.id, self.id));
        DownloadMedia::new(&self.client, location, source)
    }

    /// The photo or document is hidden behind a spoiler.
    pub fn has_media_spoiler(&self) -> bool {
        match &self.raw.media {
            Some(base::MessageMedia::MessageMediaPhoto(m)) => m.spoiler,
            Some(base::MessageMedia::MessageMediaDocument(m)) => m.spoiler,
            _ => false,
        }
    }

    /// How many seconds the photo or document stays available after it's opened.
    ///
    /// `None` if it doesn't self-destruct, and for other media.
    pub fn ttl_seconds(&self) -> Option<i32> {
        match &self.raw.media {
            Some(base::MessageMedia::MessageMediaPhoto(m)) => m.ttl_seconds,
            Some(base::MessageMedia::MessageMediaDocument(m)) => m.ttl_seconds,
            _ => None,
        }
    }

    /// Marks the contents of this message, such as a voice note, as played.
    pub async fn read_contents(&self) -> Result<()> {
        self.client.read_message_contents(self.chat.id, &[self.id]).await
    }

    /// Marks the chat as read up to and including this message.
    pub async fn mark_as_read(&self) -> Result<()> {
        match &self.business_connection_id {
            Some(id) => self.client.read_business_message(id, self.chat.id, self.id).await,
            None => self.client.read_chat_history(self.chat.id, self.id).await,
        }
    }

    /// The text or caption as HTML.
    pub fn html_text(&self) -> String {
        self.formatted().to_html()
    }

    /// The text or caption as Markdown.
    pub fn markdown_text(&self) -> String {
        self.formatted().to_markdown()
    }

    fn formatted(&self) -> Text {
        Text::from_raw(&self.raw.message, self.raw.entities.as_deref().unwrap_or_default())
    }

    /// Parses a command starting with `/` from the text or caption.
    ///
    /// Returns `None` if there is no command, or if it names another bot, as in `/start@other_bot`.
    pub fn command(&self) -> Option<Command<'_>> {
        self.command_with("/")
    }

    /// Parses a command from the text or caption, with any character of `prefixes` starting it.
    ///
    /// Returns `None` if there is no command, or if it names another bot.
    pub fn command_with(&self, prefixes: &str) -> Option<Command<'_>> {
        let command = Command::parse(self.caption.as_deref().unwrap_or(&self.text), prefixes)?;
        (command.bot.is_none() || command.is_for(self.client.0.me.lock().unwrap().username.as_deref()))
            .then_some(command)
    }

    /// Fetches the message this one replies to, on demand, or takes what `fetch_replies` fetched, or the message
    /// cache's copy when that's on; `None` when it isn't a reply or that message is gone. A business message has
    /// only the replied message its update carried.
    pub async fn reply_to_message(&self) -> Result<Option<Message>> {
        if let Some(replied) = &self.replied {
            return Ok(Some((**replied).clone()));
        }
        if let Some(reply) = self.prefetched.as_ref().and_then(|p| p.reply.as_ref()) {
            return Ok(reply.clone());
        }
        let Some(id) = self.reply_to_message_id.filter(|_| self.business_connection_id.is_none()) else {
            return Ok(None);
        };
        let cached = self.client.0.caches.message(self.chat.id, id);
        if let Some(message) = cached.and_then(|c| Message::from_raw(&self.client, c.raw, &c.entities)) {
            return Ok(Some(message));
        }
        Ok(self.client.get_messages(self.chat.id, &[id]).await?.pop().flatten())
    }

    /// Fetches this message again, as it is now; `None` when it was deleted.
    pub async fn refetch(&self) -> Result<Option<Message>> {
        // A business chat's message ids belong to the other account, so the bot can't look them up.
        if self.business_connection_id.is_some() {
            return Err(Error::InvalidArgument("a business message can't be fetched again"));
        }
        Ok(self.client.get_messages(self.chat.id, &[self.id]).await?.pop().flatten())
    }

    /// Sends `text` to this chat as a reply to this message.
    ///
    /// Awaiting the request returns the sent [`Message`]. It is [`answer`](Message::answer) with a reply to this
    /// message added.
    ///
    /// ```no_run
    /// # async fn f(message: &vaegram::types::Message) -> vaegram::Result<()> {
    /// let sent = message.reply("Got it").await?;
    /// # Ok(()) }
    /// ```
    pub fn reply(&self, text: impl Into<Text>) -> SendMessage<'_> {
        self.answer(text).reply_to(self.id)
    }

    /// Sends `text` to this chat without a quote, in the same topic and through the same business connection.
    pub fn answer(&self, text: impl Into<Text>) -> SendMessage<'_> {
        self.route(self.client.send_message(self.chat.id, text))
    }

    /// Sends an album to this chat as a reply to this message.
    ///
    /// Awaiting the request returns the sent messages.
    pub fn reply_media_group<'a>(&'a self, media: impl IntoIterator<Item = InputMedia<'a>>) -> SendMediaGroup<'a> {
        self.answer_media_group(media).reply_to(self.id)
    }

    /// Sends an album to this chat without a quote, in the same topic and through the same business connection.
    pub fn answer_media_group<'a>(&'a self, media: impl IntoIterator<Item = InputMedia<'a>>) -> SendMediaGroup<'a> {
        self.route(self.client.send_media_group(self.chat.id, media))
    }
}

/// A channel post copied into its linked discussion group: the forward was saved from its own origin.
fn is_automatic_forward(chat: &Chat, header: Option<&base::MessageFwdHeader>) -> bool {
    let Some(base::MessageFwdHeader::MessageFwdHeader(h)) = header else { return false };
    chat.kind == ChatType::Supergroup
        && matches!(h.from_id, Some(base::Peer::PeerChannel(_)))
        && h.saved_from_peer == h.from_id
        && h.channel_post.is_some()
        && h.saved_from_msg_id == h.channel_post
}

#[derive(Default)]
struct ReplyFields {
    reply_to_message_id: Option<i32>,
    reply_to_checklist_task_id: Option<i32>,
    reply_to_story_id: Option<i32>,
    reply_to_story_user_id: Option<i64>,
    reply_to_poll_option_id: Option<Vec<u8>>,
    message_thread_id: Option<i32>,
    is_topic_message: bool,
    quote: Option<TextQuote>,
    external_reply: Option<Box<ExternalReplyInfo>>,
}

fn reply_fields(
    client: &Client,
    raw: &types::Message,
    service: Option<&ServiceAction>,
    entities: &Entities,
) -> ReplyFields {
    // The message that creates a topic is its root, so the topic id is the message's own id.
    if matches!(service, Some(ServiceAction::ForumTopicCreated(_))) {
        return ReplyFields { message_thread_id: Some(raw.id), is_topic_message: true, ..Default::default() };
    }
    let reply = match &raw.reply_to {
        Some(base::MessageReplyHeader::MessageReplyHeader(reply)) => reply,
        Some(base::MessageReplyHeader::MessageReplyStoryHeader(story)) => {
            return ReplyFields {
                reply_to_story_id: Some(story.story_id),
                reply_to_story_user_id: Some(get_peer_id(&story.peer)),
                ..Default::default()
            };
        }
        None => return ReplyFields::default(),
    };
    // In a forum, a header without reply_to_top_id points at the topic itself, not at a replied message.
    let (reply_to_message_id, message_thread_id) = if reply.forum_topic && reply.reply_to_top_id.is_none() {
        (None, reply.reply_to_msg_id)
    } else {
        (reply.reply_to_msg_id.filter(|_| reply.reply_to_peer_id.is_none()), reply.reply_to_top_id)
    };
    let rich_text = reply
        .quote_rich
        .as_ref()
        .map(|texts| RichText(base::RichText::TextConcat(Box::new(types::TextConcat { texts: texts.clone() }))));
    let quote = reply.quote_text.clone().or_else(|| rich_text.as_ref().map(rich_plain)).map(|text| TextQuote {
        text,
        entities: reply.quote_entities.as_deref().and_then(|e| MessageEntity::parse_all(e, entities)),
        rich_text,
        position: reply.quote_offset.unwrap_or(0),
        is_manual: reply.quote,
    });
    ReplyFields {
        reply_to_message_id,
        reply_to_checklist_task_id: reply.todo_item_id,
        reply_to_story_id: None,
        reply_to_story_user_id: None,
        reply_to_poll_option_id: reply.poll_option.clone(),
        message_thread_id,
        is_topic_message: reply.forum_topic,
        quote,
        external_reply: ExternalReplyInfo::parse(client, reply, entities).map(Box::new),
    }
}

fn rich_plain(text: &RichText) -> String {
    let mut plain = String::new();
    push_plain(&text.0, &mut plain);
    plain
}

// The text of a media message is its caption; a link preview keeps it a text.
fn takes_caption(media: Option<&base::MessageMedia>) -> bool {
    !matches!(media, None | Some(base::MessageMedia::MessageMediaEmpty | base::MessageMedia::MessageMediaWebPage(_)))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::media::tests::document_media;
    use super::Message;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::errors::{Error, RpcError};
    use crate::filters;
    use crate::raw::{Identifiable, Serializable, base, functions, types};
    use crate::types::{
        Community, FileSource, ForumTopicCreated, ForumTopicEdited, InputMedia, Media, MessageOrigin, ServiceAction,
    };
    use crate::utils::{get_channel_id, get_peer};

    pub(crate) fn header() -> types::MessageReplyHeader {
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

    pub(crate) fn service(action: base::MessageAction) -> types::MessageService {
        types::MessageService {
            out: false,
            mentioned: false,
            media_unread: false,
            reactions_are_possible: false,
            silent: false,
            post: false,
            legacy: false,
            id: 9,
            from_id: Some(get_peer(7)),
            peer_id: get_peer(get_channel_id(55)),
            saved_peer_id: None,
            reply_to: None,
            date: 1,
            action,
            reactions: None,
            ttl_period: None,
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
        raw.entities = Some(vec![base::MessageEntity::MessageEntityBold(bold)]);
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
        assert!(!m.is_automatic_forward);
    }

    #[test]
    fn marks_channel_posts_copied_into_the_discussion_group() {
        let channel = get_peer(-1000000000003);
        let mut raw = Message::bare(1, get_peer(-1000000000005), 0);
        let base::MessageFwdHeader::MessageFwdHeader(mut header) = forward(Some(channel.clone()), None, Some(77));
        (header.saved_from_peer, header.saved_from_msg_id) = (Some(channel), Some(77));
        raw.fwd_from = Some(base::MessageFwdHeader::MessageFwdHeader(header.clone()));
        assert!(parse(&raw).is_automatic_forward);

        header.saved_from_msg_id = Some(78);
        raw.fwd_from = Some(base::MessageFwdHeader::MessageFwdHeader(header));
        assert!(!parse(&raw).is_automatic_forward);
    }

    #[tokio::test]
    async fn fetches_the_replied_message_only_when_asked() {
        let messages = types::messages::Messages {
            messages: vec![base::Message::Message(Box::new(Message::bare(4, get_peer(-5), 0)))],
            topics: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        let reply = Ok(base::messages::Messages::Messages(Box::new(messages)).to_bytes());
        let (client, mut sent) = Client::scripted(vec![reply]);
        let mut raw = Message::bare(9, get_peer(-5), 0);
        let header = types::MessageReplyHeader { reply_to_msg_id: Some(4), ..header() };
        raw.reply_to = Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(header)));
        let message = Message::parse(&client, Box::new(raw), &Entities::default());
        assert!(sent.try_recv().is_err());

        let replied = message.reply_to_message().await.unwrap().unwrap();
        assert_eq!((replied.id, replied.chat.id), (4, -5));
        let body = sent.try_recv().unwrap();
        assert_eq!(body[..4], functions::messages::GetMessages::CONSTRUCTOR_ID.to_le_bytes());

        let plain = Message::parse(&client, Box::new(Message::bare(10, get_peer(-5), 0)), &Entities::default());
        assert!(plain.reply_to_message().await.unwrap().is_none());
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_business_message_answers_with_the_replied_message_its_update_carried() {
        let (client, mut sent) = Client::recording();
        let business = |reply_to_message| {
            let mut raw = Message::bare(9, get_peer(7), 0);
            let header = types::MessageReplyHeader { reply_to_msg_id: Some(4), ..header() };
            raw.reply_to = Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(header)));
            let update = types::UpdateBotNewBusinessMessage {
                connection_id: "conn".into(),
                message: base::Message::Message(Box::new(raw)),
                reply_to_message,
                qts: 1,
            };
            let update = base::Update::UpdateBotNewBusinessMessage(Box::new(update));
            let update = types::UpdateShort { update, date: 0 };
            Message::all_from_updates(&client, base::Updates::UpdateShort(Box::new(update))).next().unwrap()
        };

        let carried = base::Message::Message(Box::new(Message::bare(4, get_peer(7), 0)));
        let replied = business(Some(carried)).reply_to_message().await.unwrap().unwrap();
        assert_eq!((replied.id, replied.business_connection_id.as_deref()), (4, Some("conn")));
        // Without it, the bot's own message ids are another account's, so nothing is fetched.
        assert!(business(None).reply_to_message().await.unwrap().is_none());
        assert!(sent.try_recv().is_err());
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
        let hub = Community {
            id: get_channel_id(7),
            name: "Hub".into(),
            date: Some(0),
            status: Some(crate::enums::CommunityMemberStatus::Member),
            ..Community::default()
        };

        let added = service(change(Some(7))).unwrap();
        assert_eq!(added.service, Some(ServiceAction::CommunityChatAdded(hub.clone())));
        assert_eq!(service(change(None)).unwrap().service, Some(ServiceAction::CommunityChatRemoved));
        let joined = types::MessageActionChatJoinedViaCommunity { community_id: 7 };
        let joined = service(base::MessageAction::MessageActionChatJoinedViaCommunity(joined)).unwrap();
        assert_eq!(joined.service, Some(ServiceAction::CommunityChatJoined(hub)));
        assert_eq!(joined.chat.id, get_channel_id(5));
        let created = types::MessageActionManagedBotCreated { bot_id: 42 };
        let created = service(base::MessageAction::MessageActionManagedBotCreated(created)).unwrap();
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
        assert!(filters::topic(9).check(&created));

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

        let pin = types::MessageReplyHeader {
            forum_topic: true,
            reply_to_msg_id: Some(12),
            reply_to_top_id: Some(4),
            ..header()
        };
        let pinned = service(Some(pin), base::MessageAction::MessageActionPinMessage);
        assert_eq!(pinned.service, Some(ServiceAction::PinnedMessage(12)));
        let topic = (pinned.message_thread_id, pinned.is_topic_message, pinned.reply_to_message_id);
        assert_eq!(topic, (Some(4), true, None));
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

        raw.media = Some(document_media("application/pdf", Vec::new()));
        let media = parse(&raw);
        assert_eq!((media.text.as_str(), media.caption.as_deref(), media.raw.message.as_str()), ("", None, ""));
        let (command, filtered) = (media.command().map(|c| c.name), filters::command("start").check(&media));
        assert_eq!((command, filtered, media.rich_message.is_some()), (None, false, true));
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
    async fn live_locations_and_polls_close_through_the_business_connection() {
        let (client, mut rx) = Client::recording();
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        let sent = [
            client.edit_message_live_location(7, 5, 1.0, 2.0).business_connection_id("c").await.map(drop),
            client.stop_message_live_location(7, 5).business_connection_id("c").await.map(drop),
            client.stop_poll(7, 5).business_connection_id("c").await.map(drop),
        ];
        for sent in sent {
            assert!(sent.is_err());
            assert_eq!(rx.try_recv().expect("no request sent")[..4], 0xdd289f8e_u32.to_le_bytes());
        }
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn business_media_is_not_looked_up_in_the_bots_own_messages() {
        let expired = Err(Error::Rpc(RpcError::new(400, "FILE_REFERENCE_EXPIRED")));
        let (client, mut rx) = Client::scripted(vec![expired]);
        client.0.me.lock().unwrap().premium = Some(false);
        let mut raw = Message::bare(5, get_peer(7), 0);
        raw.media = Some(document_media("application/pdf", Vec::new()));
        let mut message = Message::parse(&client, Box::new(raw), &Entities::default());
        message.business_connection_id = Some("c".into());
        let result = message.download().await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "FILE_REFERENCE_EXPIRED"), "{result:?}");
        assert_eq!(std::iter::from_fn(|| rx.try_recv().ok()).count(), 1);
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

    #[test]
    fn reads_spoiler_and_ttl_off_the_media() {
        let mut raw = Message::bare(1, get_peer(7), 0);
        let text = parse(&raw);
        assert_eq!((text.has_media_spoiler(), text.ttl_seconds()), (false, None));

        let photo = types::MessageMediaPhoto {
            spoiler: true,
            live_photo: false,
            photo: None,
            ttl_seconds: Some(10),
            video: None,
        };
        raw.media = Some(base::MessageMedia::MessageMediaPhoto(Box::new(photo)));
        let photo = parse(&raw);
        assert_eq!((photo.has_media_spoiler(), photo.ttl_seconds()), (true, Some(10)));

        let mut document = document_media("video/mp4", Vec::new());
        if let base::MessageMedia::MessageMediaDocument(d) = &mut document {
            (d.spoiler, d.ttl_seconds) = (true, Some(5));
        }
        raw.media = Some(document);
        let document = parse(&raw);
        assert_eq!((document.has_media_spoiler(), document.ttl_seconds()), (true, Some(5)));
    }

    const PHOTO: &str = "AgACAgIAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAIG9DEbXX3wSq3oI7t_PqQGAQADAgADbQADNgQ";

    fn seeded() -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::recording();
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        let channel = types::InputPeerChannel { channel_id: 9, access_hash: 2 };
        client.0.peers.lock().unwrap().insert(get_channel_id(9), base::InputPeer::InputPeerChannel(Box::new(channel)));
        (client, rx)
    }

    #[tokio::test]
    async fn refetch_reads_the_message_again() {
        let mut fresh = Message::bare(5, get_peer(-5), 0);
        fresh.message = "edited".into();
        let messages = types::messages::Messages {
            messages: vec![base::Message::Message(Box::new(fresh))],
            topics: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        let reply = Ok(base::messages::Messages::Messages(Box::new(messages)).to_bytes());
        let (client, mut sent) = Client::scripted(vec![reply]);
        let message = Message::parse(&client, Box::new(Message::bare(5, get_peer(-5), 0)), &Entities::default());
        let again = message.refetch().await.unwrap().unwrap();
        assert_eq!((again.id, again.chat.id, again.text.as_str()), (5, -5, "edited"));
        let request = functions::messages::GetMessages {
            id: vec![base::InputMessage::InputMessageID(types::InputMessageID { id: 5 })],
        };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn refetch_refuses_a_business_message_without_a_request() {
        let (client, mut rx) = seeded();
        let mut message = Message::parse(&client, Box::new(Message::bare(5, get_peer(7), 0)), &Entities::default());
        message.business_connection_id = Some("c".into());
        assert!(matches!(message.refetch().await, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn mark_as_read_reads_up_to_the_message() {
        let (client, mut rx) = seeded();
        let message =
            |chat_id| Message::parse(&client, Box::new(Message::bare(5, get_peer(chat_id), 0)), &Entities::default());

        assert!(message(7).mark_as_read().await.is_err());
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 1 }));
        assert_eq!(rx.try_recv().unwrap(), functions::messages::ReadHistory { peer, max_id: 5 }.to_bytes());

        assert!(message(get_channel_id(9)).mark_as_read().await.is_err());
        let channel = base::InputChannel::InputChannel(Box::new(types::InputChannel { channel_id: 9, access_hash: 2 }));
        assert_eq!(rx.try_recv().unwrap(), functions::channels::ReadHistory { channel, max_id: 5 }.to_bytes());

        let mut business = message(7);
        business.business_connection_id = Some("c".into());
        assert!(business.mark_as_read().await.is_err());
        assert_eq!(rx.try_recv().unwrap()[..4], 0xdd289f8e_u32.to_le_bytes());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn album_replies_quote_and_answers_stay_in_the_topic() {
        let (client, mut rx) = seeded();
        let album = || [0, 1].map(|_| InputMedia::photo(FileSource::file_id(PHOTO).unwrap()));
        let mut message = Message::parse(&client, Box::new(Message::bare(5, get_peer(7), 0)), &Entities::default());
        let _ = message.reply_media_group(album()).await;
        let _ = message.answer_media_group(album()).await;
        (message.message_thread_id, message.is_topic_message) = (Some(3), true);
        let _ = message.answer_media_group(album()).await;
        // reply_to is flags.0 of messages.sendMultiMedia, right after the constructor id.
        let mut has_reply_to = || {
            let body = rx.try_recv().expect("no request sent");
            assert_eq!(body[..4], functions::messages::SendMultiMedia::CONSTRUCTOR_ID.to_le_bytes());
            body[4] & 1 == 1
        };
        assert_eq!((has_reply_to(), has_reply_to(), has_reply_to()), (true, false, true));
    }

    #[tokio::test]
    async fn business_albums_go_through_the_connection() {
        let (client, mut rx) = seeded();
        let album = [0, 1].map(|_| InputMedia::photo(FileSource::file_id(PHOTO).unwrap()));
        let mut message = Message::parse(&client, Box::new(Message::bare(5, get_peer(7), 0)), &Entities::default());
        message.business_connection_id = Some("c".into());
        assert!(message.reply_media_group(album).await.is_err());
        assert_eq!(rx.try_recv().expect("no request sent")[..4], 0xdd289f8e_u32.to_le_bytes());
    }

    #[test]
    fn message_html_text_uses_text_or_caption() {
        let mut raw = Message::bare(1, get_peer(7), 0);
        raw.message = "hi <3".into();
        let bold = types::MessageEntityBold { offset: 0, length: 2 };
        raw.entities = Some(vec![base::MessageEntity::MessageEntityBold(bold)]);
        let text = parse(&raw);
        assert_eq!((text.html_text().as_str(), text.markdown_text().as_str()), ("<b>hi</b> &lt;3", "**hi** <3"));
        raw.media = Some(document_media("application/pdf", Vec::new()));
        let media = parse(&raw);
        assert_eq!((media.caption.as_deref(), media.html_text().as_str()), (Some("hi <3"), "<b>hi</b> &lt;3"));
        // A message without text or caption has nothing to format.
        assert_eq!(parse(&Message::bare(2, get_peer(7), 0)).html_text(), "");
    }

    #[test]
    fn maps_message_flags_and_counters() {
        let bare = parse(&Message::bare(1, get_peer(get_channel_id(9)), 0));
        let flags = [bare.mentioned, bare.unread_media, bare.silent, bare.channel_post, bare.from_scheduled];
        let more = [bare.edit_hidden, bare.pinned, bare.show_caption_above_media, bare.from_offline];
        assert!(flags.iter().chain(&more).all(|f| !f) && !bare.video_processing_pending);
        assert!(
            bare.sender_business_bot.is_none() && bare.sender_tag.is_none() && bare.guest_bot_caller_user.is_none()
        );
        assert_eq!(
            (bare.views, bare.forwards, bare.effect_id, bare.send_paid_messages_stars),
            (None, None, None, None)
        );
        assert_eq!((bare.repeat_period, bare.summary_language_code, bare.guest_bot_caller_chat), (None, None, None));

        let raw = types::Message {
            mentioned: true,
            media_unread: true,
            silent: true,
            post: true,
            from_scheduled: true,
            edit_hide: true,
            pinned: true,
            invert_media: true,
            offline: true,
            video_processing_pending: true,
            via_business_bot_id: Some(42),
            from_rank: Some("boss".into()),
            guestchat_via_from: Some(get_peer(43)),
            views: Some(100),
            forwards: Some(3),
            effect: Some(5_000),
            paid_message_stars: Some(25),
            schedule_repeat_period: Some(86_400),
            summary_from_language: Some("de".into()),
            ..Message::bare(1, get_peer(get_channel_id(9)), 0)
        };
        let m = parse(&raw);
        let flags = [m.mentioned, m.unread_media, m.silent, m.channel_post, m.from_scheduled, m.edit_hidden, m.pinned];
        assert!(flags.iter().all(|f| *f));
        assert!(m.show_caption_above_media && m.from_offline && m.video_processing_pending);
        assert_eq!(m.sender_business_bot.map(|u| u.id), Some(42));
        assert_eq!((m.sender_tag.as_deref(), m.guest_bot_caller_user.map(|u| u.id)), (Some("boss"), Some(43)));
        assert_eq!(
            (m.views, m.forwards, m.effect_id, m.send_paid_messages_stars),
            (Some(100), Some(3), Some(5_000), Some(25))
        );
        assert_eq!((m.repeat_period, m.summary_language_code.as_deref()), (Some(86_400), Some("de")));

        let raw = types::Message { guestchat_via_from: Some(get_peer(-77)), ..raw };
        let m = parse(&raw);
        assert_eq!((m.guest_bot_caller_user, m.guest_bot_caller_chat.map(|c| c.id)), (None, Some(-77)));
    }

    #[test]
    fn service_messages_keep_their_silent_and_unread_flags() {
        let mut raw = service(base::MessageAction::MessageActionChatDeletePhoto);
        (raw.silent, raw.media_unread) = (true, true);
        let m =
            Message::from_raw(&Client::offline(), base::Message::MessageService(Box::new(raw)), &Entities::default());
        let m = m.expect("service message dropped");
        assert_eq!((m.silent, m.unread_media), (true, true));
    }

    #[test]
    fn only_scheduling_updates_mark_a_message_scheduled() {
        let message = || base::Message::Message(Box::new(Message::bare(3, get_peer(7), 1)));
        let updates = vec![
            base::Update::UpdateNewScheduledMessage(Box::new(types::UpdateNewScheduledMessage { message: message() })),
            base::Update::UpdateNewMessage(Box::new(types::UpdateNewMessage {
                message: message(),
                pts: 1,
                pts_count: 1,
            })),
        ];
        let updates = types::Updates { updates, users: Vec::new(), chats: Vec::new(), date: 0, seq: 0 };
        let client = Client::offline();
        let found: Vec<bool> = Message::all_from_updates(&client, base::Updates::Updates(Box::new(updates)))
            .map(|m| m.scheduled)
            .collect();
        assert_eq!(found, [true, false]);
    }

    #[test]
    fn maps_story_replies_and_poll_option_replies() {
        let mut raw = Message::bare(1, get_peer(7), 0);
        let story = types::MessageReplyStoryHeader { peer: get_peer(get_channel_id(9)), story_id: 4 };
        raw.reply_to = Some(base::MessageReplyHeader::MessageReplyStoryHeader(Box::new(story)));
        let m = parse(&raw);
        assert_eq!((m.reply_to_story_id, m.reply_to_story_user_id), (Some(4), Some(get_channel_id(9))));
        assert_eq!((m.reply_to_message_id, m.message_thread_id, m.reply_to_poll_option_id), (None, None, None));

        let option = types::MessageReplyHeader { reply_to_msg_id: Some(12), poll_option: Some(vec![1]), ..header() };
        raw.reply_to = Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(option)));
        let m = parse(&raw);
        assert_eq!((m.reply_to_message_id, m.reply_to_poll_option_id), (Some(12), Some(vec![1])));
        assert_eq!((m.reply_to_story_id, m.reply_to_story_user_id), (None, None));
    }

    #[test]
    fn maps_restriction_reasons_and_fact_checks() {
        let mut raw = Message::bare(1, get_peer(get_channel_id(9)), 0);
        raw.restriction_reason = Some(Vec::new());
        let m = parse(&raw);
        assert!(m.restriction_reason.is_empty() && m.fact_check.is_none());

        let reason = types::RestrictionReason { platform: "ios".into(), reason: "porn".into(), text: "hidden".into() };
        raw.restriction_reason = Some(vec![base::RestrictionReason::RestrictionReason(Box::new(reason))]);
        let check = types::FactCheck { need_check: true, country: None, text: None, hash: 3 };
        raw.factcheck = Some(base::FactCheck::FactCheck(Box::new(check)));
        let m = parse(&raw);
        let reasons: Vec<_> = m.restriction_reason.iter().map(|r| (r.platform.as_str(), r.reason.as_str())).collect();
        assert_eq!(reasons, [("ios", "porn")]);
        assert_eq!(m.fact_check.map(|f| (f.need_check, f.hash)), Some((true, 3)));
    }

    #[test]
    fn reactions_are_none_without_a_block() {
        let mut raw = Message::bare(1, get_peer(7), 0);
        assert!(parse(&raw).reactions.is_none());
        let block = types::MessageReactions {
            min: true,
            can_see_list: false,
            reactions_as_tags: false,
            results: Vec::new(),
            recent_reactions: None,
            top_reactors: None,
        };
        raw.reactions = Some(base::MessageReactions::MessageReactions(Box::new(block)));
        let reactions = parse(&raw).reactions.expect("the reaction block was dropped");
        assert!(reactions.reactions.is_empty() && !reactions.can_get_added_reactions);

        let mut service = service(base::MessageAction::MessageActionChatDeletePhoto);
        service.reactions = raw.reactions.clone();
        let message = base::Message::MessageService(Box::new(service));
        let m = Message::from_raw(&Client::offline(), message, &Entities::default()).expect("service message dropped");
        assert!(m.reactions.is_some());
    }

    #[test]
    fn a_link_preview_keeps_the_text_and_leaves_media_empty() {
        use super::super::web_page::tests::{page, web_page_media};
        let mut raw = Message::bare(1, get_peer(7), 0);
        raw.message = "see https://example.com/v".into();
        raw.media = Some(web_page_media(base::WebPage::WebPage(Box::new(page("https://example.com/v")))));
        let m = parse(&raw);
        assert_eq!((m.text.as_str(), m.caption.as_deref()), ("see https://example.com/v", None));
        assert!(m.media.is_none());
        assert_eq!(m.web_page.map(|p| p.url), Some("https://example.com/v".into()));

        let pending = types::WebPagePending { id: 1, url: None, date: 0 };
        raw.media = Some(web_page_media(base::WebPage::WebPagePending(Box::new(pending))));
        let m = parse(&raw);
        assert!(m.web_page.is_none() && m.media.is_none() && !m.text.is_empty());
    }

    #[test]
    fn giveaway_and_story_media_have_their_accessors() {
        let mut raw = Message::bare(1, get_peer(get_channel_id(9)), 0);
        let giveaway = super::super::giveaway::tests::giveaway();
        raw.media = Some(base::MessageMedia::MessageMediaGiveaway(Box::new(giveaway)));
        let m = parse(&raw);
        assert_eq!(m.giveaway().map(|g| g.winner_count), Some(3));
        assert!(m.story().is_none() && m.giveaway_winners().is_none() && m.paid_media().is_none());

        let story = types::MessageMediaStory { via_mention: true, peer: get_peer(7), id: 4, story: None };
        raw.media = Some(base::MessageMedia::MessageMediaStory(Box::new(story)));
        assert_eq!(parse(&raw).story().map(|s| (s.chat.id, s.id)), Some((7, 4)));
    }

    #[test]
    fn imported_forward_is_import() {
        let mut raw = Message::bare(1, get_peer(7), 0);
        let base::MessageFwdHeader::MessageFwdHeader(mut header) = forward(Some(get_peer(5)), Some("Ann"), None);
        header.imported = true;
        raw.fwd_from = Some(base::MessageFwdHeader::MessageFwdHeader(header.clone()));
        let import = MessageOrigin::Import { date: 100, sender_user_name: "Ann".into() };
        assert_eq!(parse(&raw).forward_origin, Some(import));

        // The export may carry the name as the post author instead.
        (header.from_name, header.from_id, header.post_author) = (None, None, Some("Bob".into()));
        raw.fwd_from = Some(base::MessageFwdHeader::MessageFwdHeader(header));
        let import = MessageOrigin::Import { date: 100, sender_user_name: "Bob".into() };
        assert_eq!(parse(&raw).forward_origin, Some(import));

        raw.fwd_from = Some(forward(Some(get_peer(5)), None, None));
        assert!(matches!(parse(&raw).forward_origin, Some(MessageOrigin::User { .. })));
    }

    #[test]
    fn join_service_messages_keep_their_join_type() {
        use crate::enums::ChatJoinType;
        let join = |action| {
            let raw = base::Message::MessageService(Box::new(service(action)));
            Message::from_raw(&Client::offline(), raw, &Entities::default()).unwrap().chat_join_type
        };
        let add = types::MessageActionChatAddUser { users: vec![3] };
        let link = types::MessageActionChatJoinedByLink { inviter_id: 3 };
        let added = join(base::MessageAction::MessageActionChatAddUser(Box::new(add)));
        assert_eq!(added, Some(ChatJoinType::ByAdd));
        let linked = join(base::MessageAction::MessageActionChatJoinedByLink(link));
        assert_eq!(linked, Some(ChatJoinType::ByLink));
        assert_eq!(join(base::MessageAction::MessageActionChatJoinedByRequest), Some(ChatJoinType::ByRequest));
        assert_eq!(join(base::MessageAction::MessageActionChatDeletePhoto), None);
        assert_eq!(parse(&Message::bare(1, get_peer(7), 0)).chat_join_type, None);
    }

    #[test]
    fn a_rich_quote_keeps_its_text() {
        let plain = |text: &str| base::RichText::TextPlain(Box::new(types::TextPlain { text: text.into() }));
        let mut raw = Message::bare(1, get_peer(7), 0);
        let rich = types::MessageReplyHeader {
            reply_to_msg_id: Some(2),
            quote_rich: Some(vec![plain("hello "), plain("world")]),
            quote_offset: Some(1),
            ..header()
        };
        raw.reply_to = Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(rich.clone())));
        let quote = parse(&raw).quote.unwrap();
        assert_eq!((quote.text.as_str(), quote.position), ("hello world", 1));
        assert!(quote.rich_text.is_some());

        let both = types::MessageReplyHeader { quote_text: Some("hello".into()), ..rich };
        raw.reply_to = Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(both)));
        assert_eq!(parse(&raw).quote.unwrap().text, "hello");
    }
}
