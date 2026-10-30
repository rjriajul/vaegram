use crate::raw::base;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrivacyKey {
    PhoneNumber,
    AddedByPhone,
    LastSeen,
    ProfilePhoto,
    Forwards,
    PhoneCall,
    PhoneP2P,
    ChatInvite,
    VoiceMessages,
    Bio,
    Birthday,
    Gifts,
    NoPaidMessages,
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
