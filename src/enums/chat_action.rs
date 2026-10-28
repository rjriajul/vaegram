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
                A::SendMessageUploadPhotoAction(Box::new(types::SendMessageUploadPhotoAction { progress: 0 }))
            }
            ChatAction::RecordVideo => A::SendMessageRecordVideoAction,
            ChatAction::UploadVideo => {
                A::SendMessageUploadVideoAction(Box::new(types::SendMessageUploadVideoAction { progress: 0 }))
            }
            ChatAction::RecordAudio => A::SendMessageRecordAudioAction,
            ChatAction::UploadAudio => {
                A::SendMessageUploadAudioAction(Box::new(types::SendMessageUploadAudioAction { progress: 0 }))
            }
            ChatAction::UploadDocument => {
                A::SendMessageUploadDocumentAction(Box::new(types::SendMessageUploadDocumentAction { progress: 0 }))
            }
            ChatAction::FindLocation => A::SendMessageGeoLocationAction,
            ChatAction::RecordVideoNote => A::SendMessageRecordRoundAction,
            ChatAction::UploadVideoNote => {
                A::SendMessageUploadRoundAction(Box::new(types::SendMessageUploadRoundAction { progress: 0 }))
            }
            ChatAction::Playing => A::SendMessageGamePlayAction,
            ChatAction::ChooseContact => A::SendMessageChooseContactAction,
            ChatAction::ChooseSticker => A::SendMessageChooseStickerAction,
            ChatAction::Speaking => A::SpeakingInGroupCallAction,
            ChatAction::ImportHistory => {
                A::SendMessageHistoryImportAction(Box::new(types::SendMessageHistoryImportAction { progress: 0 }))
            }
            ChatAction::Cancel => A::SendMessageCancelAction,
        }
    }
}
