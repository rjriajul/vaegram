use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::ReactionsNotifySettings;

impl Client {
    /// Returns which reactions to your messages and stories, and which poll votes, notify you, and how.
    pub async fn get_reactions_notify_settings(&self) -> Result<ReactionsNotifySettings> {
        let settings = self.invoke(&functions::account::GetReactionsNotifySettings {}).await?;
        Ok(ReactionsNotifySettings::parse(settings))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::{NotificationSound, ReactionNotificationsFrom};
    use crate::raw::functions::account;
    use crate::raw::{Serializable, base, types};
    use crate::types::ReactionsNotifySettings;

    #[tokio::test]
    async fn reads_who_notifies_and_how() {
        let raw = types::ReactionsNotifySettings {
            messages_notify_from: Some(base::ReactionNotificationsFrom::ReactionNotificationsFromAll),
            stories_notify_from: None,
            poll_votes_notify_from: Some(base::ReactionNotificationsFrom::ReactionNotificationsFromContacts),
            sound: base::NotificationSound::NotificationSoundRingtone(types::NotificationSoundRingtone { id: 9 }),
            show_previews: true,
        };
        let reply = base::ReactionsNotifySettings::ReactionsNotifySettings(Box::new(raw)).to_bytes();
        let (client, mut sent) = Client::scripted(vec![Ok(reply)]);
        let settings = client.get_reactions_notify_settings().await.unwrap();
        let expected = ReactionsNotifySettings {
            messages_from: Some(ReactionNotificationsFrom::All),
            poll_votes_from: Some(ReactionNotificationsFrom::Contacts),
            sound: NotificationSound::Ringtone(9),
            show_previews: true,
            ..ReactionsNotifySettings::default()
        };
        assert_eq!(settings, expected);
        assert_eq!(sent.try_recv().unwrap(), account::GetReactionsNotifySettings {}.to_bytes());
    }
}
