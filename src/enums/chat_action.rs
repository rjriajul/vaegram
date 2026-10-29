use crate::raw::{base, types};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChatAction {
    Typing,
    UploadPhoto,
    RecordVideo,
    UploadVideo,
    RecordAudio,
    UploadAudio,
    UploadDocument,
    FindLocation,
    RecordVideoNote,
    UploadVideoNote,
    Playing,
    ChooseContact,
    ChooseSticker,
    Speaking,
    ImportHistory,
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
