use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::NotifyException;

/// Request built by [`Client::get_notify_exceptions`].
#[must_use = "a request does nothing until it is awaited"]
pub struct GetNotifyExceptions<'a> {
    client: &'a Client,
    compare_sound: bool,
    compare_stories: bool,
}

impl Client {
    /// Lists the chats and forum topics whose notification settings differ from your defaults, such as muted chats.
    ///
    /// The account-wide defaults for private chats, groups and channels aren't exceptions and are left out.
    pub fn get_notify_exceptions(&self) -> GetNotifyExceptions<'_> {
        GetNotifyExceptions { client: self, compare_sound: false, compare_stories: false }
    }
}

impl GetNotifyExceptions<'_> {
    /// Also lists chats that differ from the defaults only in their notification sound.
    pub fn compare_sound(mut self) -> Self {
        self.compare_sound = true;
        self
    }

    /// Also lists chats that differ from the defaults only in their story notification settings.
    pub fn compare_stories(mut self) -> Self {
        self.compare_stories = true;
        self
    }

    async fn send(self) -> Result<Vec<NotifyException>> {
        let request = functions::account::GetNotifyExceptions {
            compare_sound: self.compare_sound,
            compare_stories: self.compare_stories,
            peer: None,
        };
        let updates = match self.client.invoke(&request).await? {
            base::Updates::Updates(u) => u.updates,
            base::Updates::UpdatesCombined(u) => u.updates,
            base::Updates::UpdateShort(u) => vec![u.update],
            _ => Vec::new(),
        };
        Ok(updates.into_iter().filter_map(NotifyException::parse).collect())
    }
}

into_future!(GetNotifyExceptions, Vec<NotifyException>);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::functions::account;
    use crate::raw::{Serializable, base, types};
    use crate::types::{ChatNotifications, NotifyException};

    fn update(peer: base::NotifyPeer, mute_until: Option<i32>) -> base::Update {
        let settings = types::PeerNotifySettings {
            show_previews: None,
            silent: None,
            mute_until,
            ios_sound: None,
            android_sound: None,
            other_sound: None,
            stories_muted: None,
            stories_hide_sender: None,
            stories_ios_sound: None,
            stories_android_sound: None,
            stories_other_sound: None,
        };
        let update = types::UpdateNotifySettings {
            peer,
            notify_settings: base::PeerNotifySettings::PeerNotifySettings(Box::new(settings)),
        };
        base::Update::UpdateNotifySettings(Box::new(update))
    }

    fn muted(until: i32) -> ChatNotifications {
        ChatNotifications {
            show_previews: None,
            silent: None,
            mute_until: Some(until),
            stories_muted: None,
            stories_hide_sender: None,
        }
    }

    #[tokio::test]
    async fn lists_chats_and_topics_and_skips_the_defaults() {
        let chat = types::NotifyPeer { peer: base::Peer::PeerUser(types::PeerUser { user_id: 9 }) };
        let topic = types::NotifyForumTopic {
            peer: base::Peer::PeerChannel(types::PeerChannel { channel_id: 5 }),
            top_msg_id: 3,
        };
        let updates = vec![
            update(base::NotifyPeer::NotifyPeer(Box::new(chat)), Some(100)),
            update(base::NotifyPeer::NotifyUsers, Some(1)),
            update(base::NotifyPeer::NotifyForumTopic(Box::new(topic)), Some(200)),
        ];
        let reply = types::Updates { updates, users: Vec::new(), chats: Vec::new(), date: 1, seq: 0 };
        let reply = base::Updates::Updates(Box::new(reply)).to_bytes();
        let (client, mut sent) = Client::scripted_raw(vec![Ok(reply)]);

        let found = client.get_notify_exceptions().compare_sound().await.unwrap();
        let expected = [
            NotifyException { chat_id: 9, topic_id: None, notifications: muted(100) },
            NotifyException { chat_id: -1_000_000_000_005, topic_id: Some(3), notifications: muted(200) },
        ];
        assert_eq!(found, expected);
        let request = account::GetNotifyExceptions { compare_sound: true, compare_stories: false, peer: None };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }
}
