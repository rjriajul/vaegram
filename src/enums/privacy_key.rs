use crate::raw::base;

/// A privacy setting whose audience the account can change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PrivacyKey {
    /// Who can see the account's phone number.
    PhoneNumber,
    /// Who can find the account by its phone number.
    AddedByPhone,
    /// Who can see when the account was last online.
    LastSeen,
    /// Who can see the account's profile photo.
    ProfilePhoto,
    /// Who can link to the account when forwarding its messages.
    Forwards,
    /// Who can call the account.
    PhoneCall,
    /// Who can make peer-to-peer connections in calls with the account.
    PhoneP2P,
    /// Who can add the account to groups and channels.
    ChatInvite,
    /// Who can send the account voice messages.
    VoiceMessages,
    /// Who can see the account's bio.
    Bio,
    /// Who can see the account's birthday.
    Birthday,
    /// Who can send the account gifts that are saved to its profile automatically.
    Gifts,
    /// Who can message the account without paying.
    NoPaidMessages,
    /// Who can see the music saved to the account's profile.
    SavedMusic,
}

impl PrivacyKey {
    pub(crate) fn write(self) -> base::InputPrivacyKey {
        use base::InputPrivacyKey as K;
        match self {
            PrivacyKey::PhoneNumber => K::InputPrivacyKeyPhoneNumber,
            PrivacyKey::AddedByPhone => K::InputPrivacyKeyAddedByPhone,
            PrivacyKey::LastSeen => K::InputPrivacyKeyStatusTimestamp,
            PrivacyKey::ProfilePhoto => K::InputPrivacyKeyProfilePhoto,
            PrivacyKey::Forwards => K::InputPrivacyKeyForwards,
            PrivacyKey::PhoneCall => K::InputPrivacyKeyPhoneCall,
            PrivacyKey::PhoneP2P => K::InputPrivacyKeyPhoneP2P,
            PrivacyKey::ChatInvite => K::InputPrivacyKeyChatInvite,
            PrivacyKey::VoiceMessages => K::InputPrivacyKeyVoiceMessages,
            PrivacyKey::Bio => K::InputPrivacyKeyAbout,
            PrivacyKey::Birthday => K::InputPrivacyKeyBirthday,
            PrivacyKey::Gifts => K::InputPrivacyKeyStarGiftsAutoSave,
            PrivacyKey::NoPaidMessages => K::InputPrivacyKeyNoPaidMessages,
            PrivacyKey::SavedMusic => K::InputPrivacyKeySavedMusic,
        }
    }
}
