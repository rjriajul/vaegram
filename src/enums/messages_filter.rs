use crate::raw::{base, types};

/// Restricts a message search to one kind of content. The default is [`Empty`](Self::Empty).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum MessagesFilter {
    /// No filter. Every message matches.
    #[default]
    Empty,
    /// Photos.
    Photo,
    /// Videos.
    Video,
    /// Photos and videos.
    PhotoVideo,
    /// Documents sent as files.
    Document,
    /// Messages that contain a link.
    Url,
    /// Animations (GIFs).
    Animation,
    /// Voice notes.
    VoiceNote,
    /// Video notes (round videos).
    VideoNote,
    /// Both video notes and voice notes.
    AudioVideoNote,
    /// Music files.
    Audio,
    /// Service messages about a chat photo change.
    ChatPhoto,
    /// Phone call service messages. Missed calls are not singled out.
    PhoneCall,
    /// Messages that mention the current account.
    Mention,
    /// Messages that contain a location.
    Location,
    /// Messages that contain a shared contact.
    Contact,
    /// Pinned messages.
    Pinned,
    /// Polls.
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
