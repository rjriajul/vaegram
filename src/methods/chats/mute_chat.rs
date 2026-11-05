use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

/// Request built by [`Client::mute_chat`].
#[must_use = "a request does nothing until it is awaited"]
pub struct MuteChat<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    until: i32,
}

impl Client {
    /// Mutes notifications from a chat, forever unless [`mute_until`](MuteChat::mute_until) is set.
    pub fn mute_chat(&self, chat_id: impl Into<PeerRef>) -> MuteChat<'_> {
        MuteChat { client: self, chat_id: chat_id.into(), until: i32::MAX }
    }
}

impl MuteChat<'_> {
    /// Unmutes at this unix time; the default is never.
    pub fn mute_until(mut self, date: i32) -> Self {
        self.until = date;
        self
    }

    async fn send(self) -> Result<bool> {
        self.client.update_chat_notifications(self.chat_id).mute_until(self.until).await
    }
}

into_future!(MuteChat, bool);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    async fn muted(until: Option<i32>) -> Vec<u8> {
        let (client, mut rx) = Client::recording();
        let user = types::InputPeerUser { user_id: 9, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(9, base::InputPeer::InputPeerUser(Box::new(user)));
        let _ = match until {
            Some(date) => client.mute_chat(9_i64).mute_until(date).await,
            None => client.mute_chat(9_i64).await,
        };
        rx.try_recv().expect("no request sent")
    }

    fn expected(until: i32) -> Vec<u8> {
        let user = types::InputPeerUser { user_id: 9, access_hash: 1 };
        let peer = base::InputPeer::InputPeerUser(Box::new(user));
        let peer = base::InputNotifyPeer::InputNotifyPeer(Box::new(types::InputNotifyPeer { peer }));
        let settings = types::InputPeerNotifySettings {
            show_previews: None,
            silent: None,
            mute_until: Some(until),
            sound: None,
            stories_muted: None,
            stories_hide_sender: None,
            stories_sound: None,
        };
        let settings = base::InputPeerNotifySettings::InputPeerNotifySettings(Box::new(settings));
        functions::account::UpdateNotifySettings { peer, settings }.to_bytes()
    }

    #[tokio::test]
    async fn mute_until_sends_the_date() {
        assert_eq!(muted(Some(1_900_000_000)).await, expected(1_900_000_000));
    }

    #[tokio::test]
    async fn mute_defaults_to_forever() {
        assert_eq!(muted(None).await, expected(i32::MAX));
    }
}
