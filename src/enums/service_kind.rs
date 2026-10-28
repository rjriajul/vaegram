use crate::enums::ChatType;
use crate::raw::base;

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ServiceKind {
    NewChatMembers = 1,
    LeftChatMember,
    NewChatTitle,
    NewChatPhoto,
    DeleteChatPhoto,
    GroupChatCreated,
    SupergroupChatCreated,
    ChannelChatCreated,
    MigrateToChatId,
    MigrateFromChatId,
    PinnedMessage,
    AutoDeleteTimerChanged,
    VideoChatScheduled,
    VideoChatStarted,
    VideoChatEnded,
    VideoChatParticipantsInvited,
    WriteAccessAllowed,
    ConnectedWebsite,
    UsersShared,
    ChatShared,
    ProximityAlertTriggered,
    ForumTopicCreated,
    ForumTopicEdited,
    ForumTopicClosed,
    ForumTopicReopened,
    GeneralForumTopicHidden,
    GeneralForumTopicUnhidden,
    ChecklistTasksDone,
    ChecklistTasksAdded,
    CommunityChatAdded,
    CommunityChatRemoved,
    CommunityChatJoined,
    Gift,
    UniqueGift,
    ManagedBotCreated,
    SuccessfulPayment,
    RefundedPayment,
    SuggestedPostApproved,
    SuggestedPostApprovalFailed,
    SuggestedPostDeclined,
    SuggestedPostPaid,
    SuggestedPostRefunded,
    BoostAdded,
    GameHighScore,
    Unknown,
}

impl ServiceKind {
    /// `chat` tells a supergroup's creation from a channel's: both are `messageActionChannelCreate`.
    pub(crate) fn of(action: &base::MessageAction, chat: Option<ChatType>) -> ServiceKind {
        use ServiceKind as K;
        use base::MessageAction as A;
        match action {
            A::MessageActionChatAddUser(_)
            | A::MessageActionChatJoinedByLink(_)
            | A::MessageActionChatJoinedByRequest => K::NewChatMembers,
            A::MessageActionChatDeleteUser(_) => K::LeftChatMember,
            A::MessageActionChatEditTitle(_) => K::NewChatTitle,
            A::MessageActionChatEditPhoto(_) => K::NewChatPhoto,
            A::MessageActionChatDeletePhoto => K::DeleteChatPhoto,
            A::MessageActionChatCreate(_) => K::GroupChatCreated,
            A::MessageActionChannelCreate(_) if chat == Some(ChatType::Supergroup) => K::SupergroupChatCreated,
            A::MessageActionChannelCreate(_) => K::ChannelChatCreated,
            A::MessageActionChatMigrateTo(_) => K::MigrateToChatId,
            A::MessageActionChannelMigrateFrom(_) => K::MigrateFromChatId,
            A::MessageActionPinMessage => K::PinnedMessage,
            A::MessageActionSetMessagesTTL(_) => K::AutoDeleteTimerChanged,
            A::MessageActionGroupCallScheduled(_) => K::VideoChatScheduled,
            A::MessageActionGroupCall(c) if c.duration.is_some() => K::VideoChatEnded,
            A::MessageActionGroupCall(_) => K::VideoChatStarted,
            A::MessageActionInviteToGroupCall(_) => K::VideoChatParticipantsInvited,
            A::MessageActionBotAllowed(a) if a.domain.is_some() => K::ConnectedWebsite,
            A::MessageActionBotAllowed(_) => K::WriteAccessAllowed,
            A::MessageActionRequestedPeerSentMe(r)
                if r.peers.iter().all(|p| matches!(p, base::RequestedPeer::RequestedPeerUser(_))) =>
            {
                K::UsersShared
            }
            A::MessageActionRequestedPeerSentMe(_) => K::ChatShared,
            A::MessageActionRequestedPeer(r) if r.peers.iter().all(|p| matches!(p, base::Peer::PeerUser(_))) => {
                K::UsersShared
            }
            A::MessageActionRequestedPeer(_) => K::ChatShared,
            A::MessageActionGeoProximityReached(_) => K::ProximityAlertTriggered,
            A::MessageActionTopicCreate(_) => K::ForumTopicCreated,
            A::MessageActionTopicEdit(e) => match (e.closed, e.hidden) {
                (Some(true), _) => K::ForumTopicClosed,
                (Some(false), _) => K::ForumTopicReopened,
                (_, Some(true)) => K::GeneralForumTopicHidden,
                (_, Some(false)) => K::GeneralForumTopicUnhidden,
                _ => K::ForumTopicEdited,
            },
            A::MessageActionTodoCompletions(_) => K::ChecklistTasksDone,
            A::MessageActionTodoAppendTasks(_) => K::ChecklistTasksAdded,
            A::MessageActionChangeCommunity(c) if c.community_id.is_some() => K::CommunityChatAdded,
            A::MessageActionChangeCommunity(_) => K::CommunityChatRemoved,
            A::MessageActionChatJoinedViaCommunity(_) => K::CommunityChatJoined,
            // Premium, Stars and TON gifts and the payer's own receipt have no friendly type, so they stay Unknown.
            A::MessageActionStarGift(_) => K::Gift,
            A::MessageActionStarGiftUnique(_) => K::UniqueGift,
            A::MessageActionManagedBotCreated(_) => K::ManagedBotCreated,
            A::MessageActionPaymentSentMe(_) => K::SuccessfulPayment,
            A::MessageActionPaymentRefunded(_) => K::RefundedPayment,
            A::MessageActionSuggestedPostApproval(a) if a.balance_too_low => K::SuggestedPostApprovalFailed,
            A::MessageActionSuggestedPostApproval(a) if a.rejected => K::SuggestedPostDeclined,
            A::MessageActionSuggestedPostApproval(_) => K::SuggestedPostApproved,
            A::MessageActionSuggestedPostSuccess(_) => K::SuggestedPostPaid,
            A::MessageActionSuggestedPostRefund(_) => K::SuggestedPostRefunded,
            A::MessageActionBoostApply(_) => K::BoostAdded,
            A::MessageActionGameScore(_) => K::GameHighScore,
            _ => K::Unknown,
        }
    }
}
