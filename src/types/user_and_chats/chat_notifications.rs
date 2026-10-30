use crate::raw::base;

/// `None` means the chat follows the account's default for its kind of chat.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChatNotifications {
    pub show_previews: Option<bool>,
    pub silent: Option<bool>,
    pub mute_until: Option<i32>,
    pub stories_muted: Option<bool>,
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
