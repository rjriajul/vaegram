use crate::raw::{base, types};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum MessagesFilter {
    #[default]
    Empty,
    Photo,
    Video,
    PhotoVideo,
    Document,
    Url,
    Animation,
    VoiceNote,
    VideoNote,
    AudioVideoNote,
    Audio,
    ChatPhoto,
    PhoneCall,
    Mention,
    Location,
    Contact,
    Pinned,
    Poll,
}

impl MessagesFilter {
    pub(crate) fn write(self) -> base::MessagesFilter {
        use base::MessagesFilter as F;
        match self {
            Self::Empty => F::InputMessagesFilterEmpty,
            Self::Photo => F::InputMessagesFilterPhotos,
            Self::Video => F::InputMessagesFilterVideo,
            Self::PhotoVideo => F::InputMessagesFilterPhotoVideo,
            Self::Document => F::InputMessagesFilterDocument,
            Self::Url => F::InputMessagesFilterUrl,
            Self::Animation => F::InputMessagesFilterGif,
            Self::VoiceNote => F::InputMessagesFilterVoice,
            Self::VideoNote => F::InputMessagesFilterRoundVideo,
            Self::AudioVideoNote => F::InputMessagesFilterRoundVoice,
            Self::Audio => F::InputMessagesFilterMusic,
            Self::ChatPhoto => F::InputMessagesFilterChatPhotos,
            Self::PhoneCall => F::InputMessagesFilterPhoneCalls(types::InputMessagesFilterPhoneCalls { missed: false }),
            Self::Mention => F::InputMessagesFilterMyMentions,
            Self::Location => F::InputMessagesFilterGeo,
            Self::Contact => F::InputMessagesFilterContacts,
            Self::Pinned => F::InputMessagesFilterPinned,
            Self::Poll => F::InputMessagesFilterPoll,
        }
    }
}
