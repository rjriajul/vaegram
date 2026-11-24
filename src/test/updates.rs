//! Updates for a test client's handlers, built from users, chats, messages and button presses. Each compiles down to
//! the TL updates and entities Telegram would send, so handlers, filters and listeners read them as they read real
//! ones, and the users and chats they name land in the client's peer cache.

use super::State;
use super::auto_reply::{ACCESS_HASH, now};
use crate::raw::{base, types};
use crate::types::{Message, PeerId};
use crate::utils::{get_channel_id, get_peer};

/// A user an update comes from, or a peer the client should know: [`Recorder::know_user`](super::Recorder::know_user).
/// It reaches the client with an access hash, so the client can send to it.
#[derive(Clone, Debug)]
pub struct TestUser {
    id: i64,
    first_name: Option<String>,
    last_name: Option<String>,
    username: Option<String>,
    is_bot: bool,
    is_premium: bool,
}

impl TestUser {
    /// A user with this id, named "User", not a bot and without Premium.
    pub fn new(id: i64) -> Self {
        TestUser {
            id,
            first_name: Some("User".into()),
            last_name: None,
            username: None,
            is_bot: false,
            is_premium: false,
        }
    }

    /// The user's id.
    pub fn id(&self) -> i64 {
        self.id
    }

    /// Whether the user is a bot.
    pub fn is_bot(&self) -> bool {
        self.is_bot
    }

    /// Whether the user has Telegram Premium.
    pub fn is_premium(&self) -> bool {
        self.is_premium
    }

    /// Sets the first name.
    #[must_use]
    pub fn first_name(mut self, name: impl Into<String>) -> Self {
        self.first_name = Some(name.into());
        self
    }

    /// Sets the last name.
    #[must_use]
    pub fn last_name(mut self, name: impl Into<String>) -> Self {
        self.last_name = Some(name.into());
        self
    }

    /// Sets the username, without the `@`.
    #[must_use]
    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// Makes the user a bot, or not.
    #[must_use]
    pub fn bot(mut self, is_bot: bool) -> Self {
        self.is_bot = is_bot;
        self
    }

    /// Gives the user Telegram Premium, or not.
    #[must_use]
    pub fn premium(mut self, is_premium: bool) -> Self {
        self.is_premium = is_premium;
        self
    }

    pub(crate) fn raw(&self) -> base::User {
        let user = types::User {
            access_hash: Some(ACCESS_HASH),
            first_name: self.first_name.clone(),
            last_name: self.last_name.clone(),
            username: self.username.clone(),
            bot: self.is_bot,
            // `bot` and `bot_info_version` share flag 14: a bot without the version would be cut short.
            bot_info_version: self.is_bot.then_some(1),
            premium: self.is_premium,
            ..types::User::new(self.id)
        };
        base::User::User(Box::new(user))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChatKind {
    Private,
    Group,
    Supergroup,
    Channel,
}

/// The chat an update happens in. Ids are Bot API ids, as `Message::chat.id` has them: a user's id for a private
/// chat, a negative id for a basic group and `-100…` for a supergroup or a channel; a positive id given for a group,
/// supergroup or channel is turned into that form.
#[derive(Clone, Debug)]
pub struct TestChat {
    id: i64,
    kind: ChatKind,
    title: String,
    username: Option<String>,
    is_forum: bool,
}

impl TestChat {
    fn of(id: i64, kind: ChatKind) -> Self {
        TestChat { id, kind, title: "Test chat".into(), username: None, is_forum: false }
    }

    /// The private chat with the user `user_id`.
    pub fn private(user_id: i64) -> Self {
        TestChat::of(user_id, ChatKind::Private)
    }

    /// A basic group: `-chat_id`, or `chat_id` when it's negative already.
    pub fn group(chat_id: i64) -> Self {
        let id = if chat_id > 0 { chat_id.wrapping_neg() } else { chat_id };
        TestChat::of(id, ChatKind::Group)
    }

    /// A supergroup: `-100…`, from its Bot API id or its bare positive id.
    pub fn supergroup(chat_id: i64) -> Self {
        TestChat::of(channel_id(chat_id), ChatKind::Supergroup)
    }

    /// A broadcast channel: `-100…`, from its Bot API id or its bare positive id. Its messages are posts, from the
    /// channel itself and no user.
    pub fn channel(chat_id: i64) -> Self {
        TestChat::of(channel_id(chat_id), ChatKind::Channel)
    }

    /// The chat's Bot API id, as `Message::chat.id` has it.
    pub fn id(&self) -> i64 {
        self.id
    }

    /// Whether the supergroup has topics.
    pub fn is_forum(&self) -> bool {
        self.is_forum
    }

    /// Sets the title of a group, supergroup or channel.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Sets the username of a supergroup or channel, without the `@`.
    #[must_use]
    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// Makes a supergroup a forum, with topics, or not.
    #[must_use]
    pub fn forum(mut self, is_forum: bool) -> Self {
        self.is_forum = is_forum;
        self
    }

    /// The chat a Bot API id names: private for a positive id, a supergroup for `-100…`, a basic group otherwise.
    fn guess(id: i64) -> Self {
        if id > 0 {
            TestChat::private(id)
        } else if PeerId::from_bot_api(id).channel().is_some() {
            TestChat::supergroup(id)
        } else {
            TestChat::group(id)
        }
    }

    /// The TL id of a supergroup or channel.
    fn channel_tl(&self) -> Option<i64> {
        match self.kind {
            ChatKind::Supergroup | ChatKind::Channel => PeerId::from_bot_api(self.id).channel(),
            ChatKind::Private | ChatKind::Group => None,
        }
    }

    /// The chat as Telegram describes it; `None` for a private chat, which is the user.
    pub(crate) fn raw(&self) -> Option<base::Chat> {
        let photo = base::ChatPhoto::ChatPhotoEmpty;
        match self.kind {
            ChatKind::Private => None,
            ChatKind::Group => {
                let chat = types::Chat::new(self.id.wrapping_neg(), self.title.clone(), photo, 0, 0, 0);
                Some(base::Chat::Chat(Box::new(chat)))
            }
            ChatKind::Supergroup | ChatKind::Channel => {
                let id = self.channel_tl()?;
                let channel = types::Channel {
                    access_hash: Some(ACCESS_HASH),
                    megagroup: self.kind == ChatKind::Supergroup,
                    broadcast: self.kind == ChatKind::Channel,
                    forum: self.is_forum,
                    username: self.username.clone(),
                    ..types::Channel::new(id, self.title.clone(), photo, 0)
                };
                Some(base::Chat::Channel(Box::new(channel)))
            }
        }
    }
}

/// `id` as a `-100…` Bot API id; a bare positive id is taken as the channel's TL id.
fn channel_id(id: i64) -> i64 {
    if id > 0 { get_channel_id(id) } else { id }
}

/// A message to feed: new, or edited with [`TestUpdate::EditedMessage`]. Without [`id`](Self::id) it takes the
/// test client's next message id, so it comes after every message the client sent, as an answer to
/// [`Client::ask`](crate::Client::ask) must; an edit should name the message it edits with `id`.
///
/// It comes as Telegram sends it: in a private chat the sender is the chat's user, whatever [`from`](Self::from)
/// says, and in a broadcast channel it is a post from the channel, with no user.
#[derive(Clone, Debug)]
pub struct TestMessage {
    chat: TestChat,
    from: Option<TestUser>,
    text: String,
    id: Option<i32>,
    reply_to: Option<i32>,
    thread_id: Option<i32>,
    out: bool,
    date: Option<i32>,
}

impl TestMessage {
    /// `text` from the user `user_id`, in the private chat with them.
    pub fn private(user_id: i64, text: impl Into<String>) -> Self {
        TestMessage::in_chat(TestChat::private(user_id), TestUser::new(user_id), text)
    }

    /// `text` from `from`, in `chat`.
    pub fn in_chat(chat: TestChat, from: TestUser, text: impl Into<String>) -> Self {
        TestMessage {
            chat,
            from: Some(from),
            text: text.into(),
            id: None,
            reply_to: None,
            thread_id: None,
            out: false,
            date: None,
        }
    }

    /// Sets the message id; later messages without one come after it.
    #[must_use]
    pub fn id(mut self, id: i32) -> Self {
        self.id = Some(id);
        self
    }

    /// Sets the sender.
    #[must_use]
    pub fn from(mut self, user: TestUser) -> Self {
        self.from = Some(user);
        self
    }

    /// Makes it a reply to the message `message_id` of the same chat.
    #[must_use]
    pub fn reply_to(mut self, message_id: i32) -> Self {
        self.reply_to = Some(message_id);
        self
    }

    /// Puts it in the forum topic `topic_id`.
    #[must_use]
    pub fn thread_id(mut self, topic_id: i32) -> Self {
        self.thread_id = Some(topic_id);
        self
    }

    /// Makes it one the client's account sent, from another device; handlers skip those.
    #[must_use]
    pub fn outgoing(mut self, out: bool) -> Self {
        self.out = out;
        self
    }

    /// Sets the date, in Unix seconds; now unless set.
    #[must_use]
    pub fn date(mut self, date: i32) -> Self {
        self.date = Some(date);
        self
    }

    /// The users the message names: its sender, and in a private chat the chat's user.
    fn users(&self) -> Vec<base::User> {
        match self.chat.kind {
            ChatKind::Channel => Vec::new(),
            ChatKind::Private => match &self.from {
                Some(from) if from.id == self.chat.id => vec![from.raw()],
                _ => vec![TestUser::new(self.chat.id).raw()],
            },
            ChatKind::Group | ChatKind::Supergroup => self.from.iter().map(TestUser::raw).collect(),
        }
    }

    fn raw(&self, state: &State, edited: bool) -> base::Message {
        let id = match self.id {
            Some(id) => {
                state.took_message_id(id);
                id
            }
            None => state.next_message_id(),
        };
        let date = self.date.unwrap_or_else(now);
        let post = self.chat.kind == ChatKind::Channel;
        // Telegram names no sender on an incoming private message (the chat is the sender) or a channel post.
        let from_id = match self.chat.kind {
            _ if self.out => Some(state.me.id),
            ChatKind::Private | ChatKind::Channel => None,
            ChatKind::Group | ChatKind::Supergroup => self.from.as_ref().map(|u| u.id),
        };
        let reply_to = match (self.reply_to, self.thread_id) {
            (None, None) => None,
            (reply, topic) => {
                let header = types::MessageReplyHeader {
                    forum_topic: topic.is_some(),
                    reply_to_msg_id: reply.or(topic),
                    reply_to_top_id: reply.and(topic),
                    ..Default::default()
                };
                Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(header)))
            }
        };
        let message = types::Message {
            message: self.text.clone(),
            out: self.out,
            post,
            from_id: from_id.map(get_peer),
            reply_to,
            edit_date: edited.then_some(date),
            ..Message::bare(id, get_peer(self.chat.id), date)
        };
        base::Message::Message(Box::new(message))
    }

    /// The update carrying the message. A `counted` one moves the client's `pts` on, as one pushed to `run` must;
    /// one fed straight to the handlers leaves it, so a `run` going meanwhile sees no gap.
    fn update(&self, state: &State, edited: bool, counted: bool) -> base::Update {
        let message = self.raw(state, edited);
        let pts_count = i32::from(counted);
        match (self.chat.channel_tl(), edited) {
            (Some(channel), edited) => {
                let pts = if counted { state.next_channel_pts(channel) } else { state.channel_pts(channel) };
                if edited {
                    let update = types::UpdateEditChannelMessage { message, pts, pts_count };
                    base::Update::UpdateEditChannelMessage(Box::new(update))
                } else {
                    let update = types::UpdateNewChannelMessage { message, pts, pts_count };
                    base::Update::UpdateNewChannelMessage(Box::new(update))
                }
            }
            (None, edited) => {
                let pts = if counted { state.next_pts() } else { state.pts() };
                if edited {
                    base::Update::UpdateEditMessage(Box::new(types::UpdateEditMessage { message, pts, pts_count }))
                } else {
                    base::Update::UpdateNewMessage(Box::new(types::UpdateNewMessage { message, pts, pts_count }))
                }
            }
        }
    }
}

/// A press of an inline keyboard button under the message `message_id` in the chat `chat_id` (a Bot API id; a
/// `-100…` id is taken as a supergroup unless [`chat`](Self::chat) says otherwise). Without
/// [`query_id`](Self::query_id) it takes the next id from 1 up.
#[derive(Clone, Debug)]
pub struct TestCallbackQuery {
    from: TestUser,
    chat: TestChat,
    message_id: i32,
    data: String,
    query_id: Option<i64>,
}

impl TestCallbackQuery {
    /// The user `user_id` pressing the button with `data` under the message `message_id` in `chat_id`.
    pub fn new(user_id: i64, chat_id: i64, message_id: i32, data: impl Into<String>) -> Self {
        TestCallbackQuery {
            from: TestUser::new(user_id),
            chat: TestChat::guess(chat_id),
            message_id,
            data: data.into(),
            query_id: None,
        }
    }

    /// Sets the query id, which [`CallKind::AnswerCallbackQuery`](super::CallKind::AnswerCallbackQuery) names.
    #[must_use]
    pub fn query_id(mut self, id: i64) -> Self {
        self.query_id = Some(id);
        self
    }

    /// Sets the user who pressed it.
    #[must_use]
    pub fn from(mut self, user: TestUser) -> Self {
        self.from = user;
        self
    }

    /// Sets the chat of the message, such as a broadcast channel.
    #[must_use]
    pub fn chat(mut self, chat: TestChat) -> Self {
        self.chat = chat;
        self
    }
}

/// One update for [`Recorder::feed`](super::Recorder::feed), [`dispatch`](super::Recorder::dispatch) or
/// [`push_stream`](super::Recorder::push_stream).
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum TestUpdate {
    /// A new message.
    Message(TestMessage),
    /// An edited message; give it the id of the message it edits with [`TestMessage::id`].
    EditedMessage(TestMessage),
    /// A press of an inline keyboard button.
    CallbackQuery(TestCallbackQuery),
    /// These TL updates as they are, entities included. `feed` and `dispatch` take `updates`, `updatesCombined`
    /// and `updateShort`, and drop the short message forms and `updatesTooLong`, which only the tracker reads:
    /// push those with `push_stream`.
    ///
    /// Experimental (`unstable-raw`): may change or go away in any release.
    #[cfg(feature = "unstable-raw")]
    Raw(base::Updates),
}

impl From<TestMessage> for TestUpdate {
    fn from(message: TestMessage) -> Self {
        TestUpdate::Message(message)
    }
}

impl From<TestCallbackQuery> for TestUpdate {
    fn from(query: TestCallbackQuery) -> Self {
        TestUpdate::CallbackQuery(query)
    }
}

impl TestUpdate {
    /// The TL updates, with the users and chats they name; message ids and `pts` come from `state`, the `pts`
    /// moved on when `counted` (see [`TestMessage::update`]).
    pub(crate) fn into_raw(self, state: &State, counted: bool) -> base::Updates {
        let (update, users, chat) = match self {
            TestUpdate::Message(m) => (m.update(state, false, counted), m.users(), m.chat),
            TestUpdate::EditedMessage(m) => (m.update(state, true, counted), m.users(), m.chat),
            TestUpdate::CallbackQuery(q) => {
                let query_id = q.query_id.unwrap_or_else(|| state.next_query_id());
                let update = types::UpdateBotCallbackQuery {
                    data: Some(q.data.into_bytes()),
                    ..types::UpdateBotCallbackQuery::new(
                        query_id,
                        q.from.id,
                        get_peer(q.chat.id),
                        q.message_id,
                        q.chat.id,
                    )
                };
                let mut users = vec![q.from.raw()];
                if q.chat.kind == ChatKind::Private && q.chat.id != q.from.id {
                    users.push(TestUser::new(q.chat.id).raw());
                }
                (base::Update::UpdateBotCallbackQuery(Box::new(update)), users, q.chat)
            }
            #[cfg(feature = "unstable-raw")]
            TestUpdate::Raw(updates) => return updates,
        };
        let updates = types::Updates {
            updates: vec![update],
            users,
            chats: chat.raw().into_iter().collect(),
            date: now(),
            seq: 0,
        };
        base::Updates::Updates(Box::new(updates))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chats_take_bot_api_ids() {
        assert_eq!(TestChat::group(5).id(), -5);
        assert_eq!(TestChat::group(-5).id(), -5);
        assert_eq!(TestChat::supergroup(1234).id(), get_channel_id(1234));
        assert_eq!(TestChat::channel(get_channel_id(1234)).channel_tl(), Some(1234));
        assert_eq!(TestChat::guess(get_channel_id(7)).kind, ChatKind::Supergroup);
        assert_eq!(TestChat::guess(-7).kind, ChatKind::Group);
    }

    #[test]
    fn senders_are_named_as_telegram_names_them() {
        let user = TestUser::new(42);
        let private = TestMessage::private(42, "hi");
        let channel = TestMessage::in_chat(TestChat::channel(5), user.clone(), "post");
        let group = TestMessage::in_chat(TestChat::group(5), user, "hi");
        assert_eq!(private.users().len(), 1);
        assert!(channel.users().is_empty());
        assert_eq!(group.users().len(), 1);
    }
}
