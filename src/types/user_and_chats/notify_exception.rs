use crate::raw::base;
use crate::types::ChatNotifications;
use crate::utils::get_peer_id;

/// A chat, or a forum topic, whose notification settings differ from the account's defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct NotifyException {
    /// Id of the chat.
    pub chat_id: i64,
    /// Id of the forum topic when the exception is for one topic only, not the whole chat.
    pub topic_id: Option<i32>,
    /// The settings that apply there.
    pub notifications: ChatNotifications,
}

impl NotifyException {
    /// The exception an `updateNotifySettings` carries. `None` for any other update and for the account-wide
    /// defaults of private chats, groups and channels, which are no exception to anything.
    pub(crate) fn parse(update: base::Update) -> Option<NotifyException> {
        let base::Update::UpdateNotifySettings(update) = update else { return None };
        let update = *update;
        let (chat_id, topic_id) = match update.peer {
            base::NotifyPeer::NotifyPeer(peer) => (get_peer_id(&peer.peer), None),
            base::NotifyPeer::NotifyForumTopic(topic) => (get_peer_id(&topic.peer), Some(topic.top_msg_id)),
            _ => return None,
        };
        Some(NotifyException { chat_id, topic_id, notifications: ChatNotifications::parse(update.notify_settings) })
    }
}
