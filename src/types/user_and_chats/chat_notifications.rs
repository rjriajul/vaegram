use crate::raw::base;

/// Notification settings for one chat.
///
/// A `None` field means the chat follows the account's default for its kind of chat.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChatNotifications {
    /// Whether notifications show a message preview.
    pub show_previews: Option<bool>,
    /// Whether notifications arrive without sound.
    pub silent: Option<bool>,
    /// Unix time the mute ends.
    pub mute_until: Option<i32>,
    /// Whether stories from this chat are muted.
    pub stories_muted: Option<bool>,
    /// Whether story notifications hide the sender's name.
    pub stories_hide_sender: Option<bool>,
}

impl ChatNotifications {
    pub(crate) fn parse(raw: base::PeerNotifySettings) -> ChatNotifications {
        let base::PeerNotifySettings::PeerNotifySettings(s) = raw;
        ChatNotifications {
            show_previews: s.show_previews,
            silent: s.silent,
            mute_until: s.mute_until,
            stories_muted: s.stories_muted,
            stories_hide_sender: s.stories_hide_sender,
        }
    }
}
