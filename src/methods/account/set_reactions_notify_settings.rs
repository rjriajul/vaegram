use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::ReactionsNotifySettings;

impl Client {
    /// Replaces which reactions and poll votes notify you, and how, and returns the settings Telegram stored.
    ///
    /// Every setting is replaced; to change one, start from
    /// [`get_reactions_notify_settings`](Client::get_reactions_notify_settings).
    pub async fn set_reactions_notify_settings(
        &self,
        settings: ReactionsNotifySettings,
    ) -> Result<ReactionsNotifySettings> {
        let request = functions::account::SetReactionsNotifySettings { settings: settings.write() };
        Ok(ReactionsNotifySettings::parse(self.invoke(&request).await?))
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
    async fn sends_the_settings_and_returns_the_stored_ones() {
        let settings = ReactionsNotifySettings {
            messages_from: Some(ReactionNotificationsFrom::All),
            poll_votes_from: Some(ReactionNotificationsFrom::Contacts),
            sound: NotificationSound::Silent,
            show_previews: true,
            ..ReactionsNotifySettings::default()
        };

        let raw = types::ReactionsNotifySettings {
            messages_notify_from: Some(base::ReactionNotificationsFrom::ReactionNotificationsFromAll),
            stories_notify_from: None,
            poll_votes_notify_from: Some(base::ReactionNotificationsFrom::ReactionNotificationsFromContacts),
            sound: base::NotificationSound::NotificationSoundNone,
            show_previews: true,
        };
        let raw = base::ReactionsNotifySettings::ReactionsNotifySettings(Box::new(raw));
        let (client, mut sent) = Client::scripted_raw(vec![Ok(raw.to_bytes())]);
        assert_eq!(client.set_reactions_notify_settings(settings.clone()).await.unwrap(), settings);
        assert_eq!(sent.try_recv().unwrap(), account::SetReactionsNotifySettings { settings: raw }.to_bytes());
    }
}
