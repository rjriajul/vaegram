use crate::enums::{NotificationSound, ReactionNotificationsFrom};
use crate::raw::{base, types};

/// Which reactions and poll votes raise a notification, and how.
///
/// To change one setting, read the current ones with
/// [`get_reactions_notify_settings`](crate::Client::get_reactions_notify_settings), change the field and send the
/// result back with [`set_reactions_notify_settings`](crate::Client::set_reactions_notify_settings).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ReactionsNotifySettings {
    /// Whose reactions to your messages notify you. `None` turns these notifications off.
    pub messages_from: Option<ReactionNotificationsFrom>,
    /// Whose reactions to your stories notify you. `None` turns these notifications off.
    pub stories_from: Option<ReactionNotificationsFrom>,
    /// Whose votes in your polls notify you. `None` turns these notifications off.
    pub poll_votes_from: Option<ReactionNotificationsFrom>,
    /// The sound of these notifications.
    pub sound: NotificationSound,
    /// Whether these notifications show a preview of the message.
    pub show_previews: bool,
}

impl ReactionsNotifySettings {
    pub(crate) fn parse(raw: base::ReactionsNotifySettings) -> ReactionsNotifySettings {
        let base::ReactionsNotifySettings::ReactionsNotifySettings(settings) = raw;
        let settings = *settings;
        ReactionsNotifySettings {
            messages_from: settings.messages_notify_from.map(ReactionNotificationsFrom::parse),
            stories_from: settings.stories_notify_from.map(ReactionNotificationsFrom::parse),
            poll_votes_from: settings.poll_votes_notify_from.map(ReactionNotificationsFrom::parse),
            sound: NotificationSound::parse(settings.sound),
            show_previews: settings.show_previews,
        }
    }

    pub(crate) fn write(self) -> base::ReactionsNotifySettings {
        base::ReactionsNotifySettings::ReactionsNotifySettings(Box::new(types::ReactionsNotifySettings {
            messages_notify_from: self.messages_from.map(ReactionNotificationsFrom::write),
            stories_notify_from: self.stories_from.map(ReactionNotificationsFrom::write),
            poll_votes_notify_from: self.poll_votes_from.map(ReactionNotificationsFrom::write),
            sound: self.sound.write(),
            show_previews: self.show_previews,
        }))
    }
}
