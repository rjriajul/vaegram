use crate::raw::{base, types};

/// The sound a notification plays.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum NotificationSound {
    /// The app's default sound.
    #[default]
    Default,
    /// No sound.
    Silent,
    /// A sound that ships with the app, named by `title` and `data`.
    Local {
        /// Name of the sound as the app shows it.
        title: String,
        /// Identifier of the sound inside the app.
        data: String,
    },
    /// A ringtone saved to the account, by the id of its document.
    Ringtone(i64),
}

impl NotificationSound {
    pub(crate) fn parse(raw: base::NotificationSound) -> NotificationSound {
        match raw {
            base::NotificationSound::NotificationSoundDefault => NotificationSound::Default,
            base::NotificationSound::NotificationSoundNone => NotificationSound::Silent,
            base::NotificationSound::NotificationSoundLocal(sound) => {
                let sound = *sound;
                NotificationSound::Local { title: sound.title, data: sound.data }
            }
            base::NotificationSound::NotificationSoundRingtone(sound) => NotificationSound::Ringtone(sound.id),
        }
    }

    pub(crate) fn write(self) -> base::NotificationSound {
        match self {
            NotificationSound::Default => base::NotificationSound::NotificationSoundDefault,
            NotificationSound::Silent => base::NotificationSound::NotificationSoundNone,
            NotificationSound::Local { title, data } => {
                let sound = types::NotificationSoundLocal { title, data };
                base::NotificationSound::NotificationSoundLocal(Box::new(sound))
            }
            NotificationSound::Ringtone(id) => {
                base::NotificationSound::NotificationSoundRingtone(types::NotificationSoundRingtone { id })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NotificationSound;

    #[test]
    fn every_sound_survives_a_round_trip() {
        let local = NotificationSound::Local { title: "Chime".into(), data: "chime".into() };
        for sound in [NotificationSound::Default, NotificationSound::Silent, local, NotificationSound::Ringtone(7)] {
            assert_eq!(NotificationSound::parse(sound.clone().write()), sound);
        }
    }
}
