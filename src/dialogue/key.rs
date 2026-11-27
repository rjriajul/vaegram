use std::fmt;
use std::sync::Arc;

use crate::types::Message;

/// Whose conversation an update belongs to: what a [`DialogueRouter`](super::DialogueRouter) keys its state by.
///
/// In a private chat the chat is the user, so every scope but `Chat` and `ChatTopic` behaves the same there. They
/// differ in groups: with the default, [`UserInChat`](Self::UserInChat), two members filling in the same form at
/// once each have their own state. Under every scope, updates that come through a business connection are apart
/// from the bot's own: a customer's chat with the business is the same chat id as their chat with the bot, but
/// never the same conversation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Scope {
    /// One state per user in each chat. A message sent on behalf of a chat (an anonymous admin, a channel) counts
    /// as that chat's.
    #[default]
    UserInChat,
    /// One state per chat, shared by everyone in it.
    Chat,
    /// One state per user, followed across every chat, inline buttons included.
    User,
    /// One state per user in each forum topic; outside topics, as [`UserInChat`](Self::UserInChat).
    UserInTopic,
    /// One state per forum topic, shared by everyone in it; outside topics, as [`Chat`](Self::Chat).
    ChatTopic,
}

impl Scope {
    /// The key for this chat, user, topic and business connection, each `None` when the update has none. Inline
    /// buttons have no chat.
    pub(crate) fn key(
        self,
        namespace: &Arc<str>,
        chat: Option<i64>,
        user: Option<i64>,
        thread: Option<i32>,
        business: Option<&str>,
    ) -> Option<DialogueKey> {
        let (chat_id, user_id, thread) = match self {
            Scope::UserInChat => (chat?, user?, None),
            Scope::Chat => (chat?, 0, None),
            Scope::User => (0, user?, None),
            Scope::UserInTopic => (chat?, user?, thread),
            Scope::ChatTopic => (chat?, 0, thread),
        };
        let key = DialogueKey::new(Arc::clone(namespace), chat_id, user_id);
        Some(key.thread(thread).business_connection(business))
    }

    /// Whether keys of this scope name a forum topic.
    pub(crate) fn by_topic(self) -> bool {
        matches!(self, Scope::UserInTopic | Scope::ChatTopic)
    }
}

/// Who sent `message`: its user, else the chat it was sent on behalf of, else the user of a private chat.
pub(crate) fn sender(message: &Message) -> Option<i64> {
    if let Some(user) = &message.from_user {
        return Some(user.id);
    }
    if let Some(chat) = &message.sender_chat {
        return Some(chat.id);
    }
    (message.chat.id > 0).then_some(message.chat.id)
}

/// The forum topic `message` is in, if any.
pub(crate) fn topic(message: &Message) -> Option<i32> {
    if message.is_topic_message { message.message_thread_id } else { None }
}

/// Names one conversation's state in a [`DialogueStorage`](super::DialogueStorage): the dialogue's namespace, the
/// chat, the user, the forum topic and the business connection. A scope leaves out what it doesn't key by, as `0`
/// (or no topic).
///
/// Its [`Display`](fmt::Display) form is `chat:user:topic:business:namespace`, with `0` for no topic, `-` for no
/// business connection and `<length>.<id>` for one. It is unambiguous whatever the connection id and the namespace
/// hold, so a storage may use it as its own key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DialogueKey {
    namespace: Arc<str>,
    chat_id: i64,
    user_id: i64,
    thread_id: Option<i32>,
    business_connection_id: Option<Arc<str>>,
}

impl DialogueKey {
    /// The key of `user_id` in `chat_id` in the dialogue `namespace`, outside any topic or business connection. Use
    /// `0` for a part the dialogue's [`Scope`] doesn't key by: `user_id` with [`Scope::Chat`], `chat_id` with
    /// [`Scope::User`].
    pub fn new(namespace: impl Into<Arc<str>>, chat_id: i64, user_id: i64) -> Self {
        DialogueKey { namespace: namespace.into(), chat_id, user_id, thread_id: None, business_connection_id: None }
    }

    /// The same key in the forum topic `thread_id`, or outside topics for `None`. Topic ids are message ids, so
    /// `Some(0)` names no topic and is taken as `None`.
    #[must_use]
    pub fn thread(mut self, thread_id: Option<i32>) -> Self {
        self.thread_id = thread_id.filter(|&id| id != 0);
        self
    }

    /// The same key through the business connection `id`, or outside business connections for `None`.
    #[must_use]
    pub fn business_connection(mut self, id: Option<&str>) -> Self {
        self.business_connection_id = id.map(Arc::from);
        self
    }

    /// The dialogue's namespace.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// The chat, by Bot-API id; `0` when the scope doesn't key by chat.
    pub fn chat_id(&self) -> i64 {
        self.chat_id
    }

    /// The user (or the chat a message was sent on behalf of); `0` when the scope doesn't key by user.
    pub fn user_id(&self) -> i64 {
        self.user_id
    }

    /// The forum topic; `None` outside topics or when the scope doesn't key by topic.
    pub fn thread_id(&self) -> Option<i32> {
        self.thread_id
    }

    /// The business connection the conversation goes through; `None` for the bot's own chats.
    pub fn business_connection_id(&self) -> Option<&str> {
        self.business_connection_id.as_deref()
    }
}

impl fmt::Display for DialogueKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let thread = self.thread_id.unwrap_or(0);
        write!(f, "{}:{}:{}:", self.chat_id, self.user_id, thread)?;
        match &self.business_connection_id {
            Some(id) => write!(f, "{}.{id}:", id.len())?,
            None => f.write_str("-:")?,
        }
        f.write_str(&self.namespace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ns() -> Arc<str> {
        Arc::from("signup")
    }

    #[test]
    fn each_scope_keys_by_its_own_parts() {
        let (chat, user, topic) = (Some(-1005), Some(7), Some(3));
        let key = |scope: Scope| scope.key(&ns(), chat, user, topic, None).unwrap();
        assert_eq!(key(Scope::UserInChat), DialogueKey::new("signup", -1005, 7));
        assert_eq!(key(Scope::Chat), DialogueKey::new("signup", -1005, 0));
        assert_eq!(key(Scope::User), DialogueKey::new("signup", 0, 7));
        assert_eq!(key(Scope::UserInTopic), DialogueKey::new("signup", -1005, 7).thread(Some(3)));
        assert_eq!(key(Scope::ChatTopic), DialogueKey::new("signup", -1005, 0).thread(Some(3)));
    }

    #[test]
    fn a_missing_part_the_scope_needs_gives_no_key() {
        // An inline button has no chat; a channel post has no user.
        assert_eq!(Scope::UserInChat.key(&ns(), None, Some(7), None, None), None);
        assert_eq!(Scope::Chat.key(&ns(), None, Some(7), None, None), None);
        assert_eq!(Scope::User.key(&ns(), Some(-5), None, None, None), None);
        assert!(Scope::User.key(&ns(), None, Some(7), None, None).is_some());
        assert!(Scope::Chat.key(&ns(), Some(-5), None, None, None).is_some());
    }

    #[test]
    fn a_business_connection_is_its_own_conversation_under_every_scope() {
        let scopes = [Scope::UserInChat, Scope::Chat, Scope::User, Scope::UserInTopic, Scope::ChatTopic];
        for scope in scopes {
            let own = scope.key(&ns(), Some(42), Some(42), None, None).unwrap();
            let business = scope.key(&ns(), Some(42), Some(42), None, Some("conn")).unwrap();
            assert_ne!(own, business, "{scope:?}");
            assert_ne!(own.to_string(), business.to_string(), "{scope:?}");
            assert_eq!(business.business_connection_id(), Some("conn"));
        }
    }

    #[test]
    fn display_cannot_collide_whatever_the_ids_and_namespace_hold() {
        // A namespace or connection id holding the separator must not read as other parts.
        let odd = DialogueKey::new("a:1", 2, 3);
        let plain = DialogueKey::new("a", 1, 2).thread(Some(3));
        assert_eq!(odd.to_string(), "2:3:0:-:a:1");
        assert_eq!(plain.to_string(), "1:2:3:-:a");
        let tricky = DialogueKey::new("b", 1, 1).business_connection(Some("x:-"));
        let other = DialogueKey::new("-:b", 1, 1).business_connection(Some("x"));
        assert_eq!(tricky.to_string(), "1:1:0:3.x:-:b");
        assert_eq!(other.to_string(), "1:1:0:1.x:-:b");
        assert_ne!(tricky.to_string(), other.to_string());
    }

    #[test]
    fn topic_zero_is_no_topic_for_equality_and_display_alike() {
        let zero = DialogueKey::new("a", 1, 2).thread(Some(0));
        let none = DialogueKey::new("a", 1, 2).thread(None);
        assert_eq!(zero, none);
        assert_eq!(zero.thread_id(), None);
        assert_eq!(zero.to_string(), none.to_string());
    }
}
