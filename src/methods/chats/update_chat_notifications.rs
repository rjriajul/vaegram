use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::PeerRef;

/// Request built by [`Client::update_chat_notifications`].
#[must_use = "a request does nothing until it is awaited"]
pub struct UpdateChatNotifications<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    settings: types::InputPeerNotifySettings,
}

impl Client {
    /// Changes the notification settings of a chat.
    ///
    /// Only the settings set on the builder change.
    pub fn update_chat_notifications(&self, chat_id: impl Into<PeerRef>) -> UpdateChatNotifications<'_> {
        let settings = types::InputPeerNotifySettings {
            show_previews: None,
            silent: None,
            mute_until: None,
            sound: None,
            stories_muted: None,
            stories_hide_sender: None,
            stories_sound: None,
        };
        UpdateChatNotifications { client: self, chat_id: chat_id.into(), settings }
    }
}

impl UpdateChatNotifications<'_> {
    /// Mutes the chat until this unix time. `0` unmutes it.
    pub fn mute_until(mut self, date: i32) -> Self {
        self.settings.mute_until = Some(date);
        self
    }

    /// Delivers the chat's notifications without sound.
    pub fn silent(mut self, silent: bool) -> Self {
        self.settings.silent = Some(silent);
        self
    }

    /// Shows message text in the chat's notifications.
    pub fn show_previews(mut self, show: bool) -> Self {
        self.settings.show_previews = Some(show);
        self
    }

    /// Mutes notifications for the chat's stories.
    pub fn stories_muted(mut self, muted: bool) -> Self {
        self.settings.stories_muted = Some(muted);
        self
    }

    /// Hides the sender in notifications for the chat's stories.
    pub fn stories_hide_sender(mut self, hide: bool) -> Self {
        self.settings.stories_hide_sender = Some(hide);
        self
    }

    async fn send(self) -> Result<()> {
        let peer = base::InputNotifyPeer::InputNotifyPeer(Box::new(types::InputNotifyPeer {
            peer: self.client.resolve_peer(self.chat_id).await?,
        }));
        let settings = base::InputPeerNotifySettings::InputPeerNotifySettings(Box::new(self.settings));
        self.client.invoke(&functions::account::UpdateNotifySettings { peer, settings }).await?;
        Ok(())
    }
}

into_future!(UpdateChatNotifications, ());
