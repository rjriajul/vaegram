use crate::raw::{base, types};

/// An activity shown to other members of a chat, such as "typing...". Upload actions are sent with progress 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChatAction {
    /// Typing a text message.
    Typing,
    /// Uploading a photo.
    UploadPhoto,
    /// Recording a video.
    RecordVideo,
    /// Uploading a video.
    UploadVideo,
    /// Recording a voice message.
    RecordAudio,
    /// Uploading a voice message.
    UploadAudio,
    /// Uploading a file.
    UploadDocument,
    /// Choosing a location to send.
    FindLocation,
    /// Recording a video note.
    RecordVideoNote,
    /// Uploading a video note.
    UploadVideoNote,
    /// Playing a game.
    Playing,
    /// Choosing a contact to send.
    ChooseContact,
    /// Choosing a sticker to send.
    ChooseSticker,
    /// Speaking in a group call.
    Speaking,
    /// Importing a chat history.
    ImportHistory,
    /// Clears the action that was showing.
    Cancel,
}

impl ChatAction {
    pub(crate) fn write(self) -> base::SendMessageAction {
        use base::SendMessageAction as A;
        match self {
            ChatAction::Typing => A::SendMessageTypingAction,
            ChatAction::UploadPhoto => {
                A::SendMessageUploadPhotoAction(types::SendMessageUploadPhotoAction { progress: 0 })
            }
            ChatAction::RecordVideo => A::SendMessageRecordVideoAction,
            ChatAction::UploadVideo => {
                A::SendMessageUploadVideoAction(types::SendMessageUploadVideoAction { progress: 0 })
            }
            ChatAction::RecordAudio => A::SendMessageRecordAudioAction,
            ChatAction::UploadAudio => {
                A::SendMessageUploadAudioAction(types::SendMessageUploadAudioAction { progress: 0 })
            }
            ChatAction::UploadDocument => {
                A::SendMessageUploadDocumentAction(types::SendMessageUploadDocumentAction { progress: 0 })
            }
            ChatAction::FindLocation => A::SendMessageGeoLocationAction,
            ChatAction::RecordVideoNote => A::SendMessageRecordRoundAction,
            ChatAction::UploadVideoNote => {
                A::SendMessageUploadRoundAction(types::SendMessageUploadRoundAction { progress: 0 })
            }
            ChatAction::Playing => A::SendMessageGamePlayAction,
            ChatAction::ChooseContact => A::SendMessageChooseContactAction,
            ChatAction::ChooseSticker => A::SendMessageChooseStickerAction,
            ChatAction::Speaking => A::SpeakingInGroupCallAction,
            ChatAction::ImportHistory => {
                A::SendMessageHistoryImportAction(types::SendMessageHistoryImportAction { progress: 0 })
            }
            ChatAction::Cancel => A::SendMessageCancelAction,
        }
    }
}
