use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
pub struct UpdateChatNotifications<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    settings: types::InputPeerNotifySettings,
}

impl Client {
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
    pub fn mute_until(mut self, date: i32) -> Self {
        self.settings.mute_until = Some(date);
        self
    }

    pub fn silent(mut self, silent: bool) -> Self {
        self.settings.silent = Some(silent);
        self
    }

    pub fn show_previews(mut self, show: bool) -> Self {
        self.settings.show_previews = Some(show);
        self
    }

    pub fn stories_muted(mut self, muted: bool) -> Self {
        self.settings.stories_muted = Some(muted);
        self
    }

    pub fn stories_hide_sender(mut self, hide: bool) -> Self {
        self.settings.stories_hide_sender = Some(hide);
        self
    }

    async fn send(self) -> Result<bool> {
        let peer = base::InputNotifyPeer::InputNotifyPeer(Box::new(types::InputNotifyPeer {
            peer: self.client.resolve_peer(self.chat_id).await?,
        }));
        let settings = base::InputPeerNotifySettings::InputPeerNotifySettings(Box::new(self.settings));
        self.client.invoke(&functions::account::UpdateNotifySettings { peer, settings }).await
    }
}

into_future!(UpdateChatNotifications, bool);
