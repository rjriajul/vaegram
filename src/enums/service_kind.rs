use crate::enums::ChatType;
use crate::raw::base;

/// What a service message reports, such as a member joining or a pinned message.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ServiceKind {
    /// Members were added, or joined by invite link or join request.
    NewChatMembers = 1,
    /// A member left or was removed.
    LeftChatMember,
    /// The chat title changed.
    NewChatTitle,
    /// The chat photo changed.
    NewChatPhoto,
    /// The chat photo was removed.
    DeleteChatPhoto,
    /// A basic group was created.
    GroupChatCreated,
    /// A supergroup was created.
    SupergroupChatCreated,
    /// A channel was created. Also reported for a supergroup when the chat's type isn't known.
    ChannelChatCreated,
    /// The group was upgraded to a supergroup.
    MigrateToChatId,
    /// The supergroup was created from a basic group.
    MigrateFromChatId,
    /// A message was pinned.
    PinnedMessage,
    /// The auto-delete timer changed.
    AutoDeleteTimerChanged,
    /// A video chat was scheduled.
    VideoChatScheduled,
    /// A video chat started.
    VideoChatStarted,
    /// A video chat ended.
    VideoChatEnded,
    /// Participants were invited to a video chat.
    VideoChatParticipantsInvited,
    /// A bot was allowed to write to the user.
    WriteAccessAllowed,
    /// The user logged in to a website through a bot.
    ConnectedWebsite,
    /// The user shared users with a bot.
    UsersShared,
    /// The user shared a chat with a bot.
    ChatShared,
    /// A proximity alert for a shared live location fired.
    ProximityAlertTriggered,
    /// A forum topic was created.
    ForumTopicCreated,
    /// A forum topic was edited.
    ForumTopicEdited,
    /// A forum topic was closed.
    ForumTopicClosed,
    /// A forum topic was reopened.
    ForumTopicReopened,
    /// The General forum topic was hidden.
    GeneralForumTopicHidden,
    /// The General forum topic was shown again.
    GeneralForumTopicUnhidden,
    /// Checklist tasks were marked done or undone.
    ChecklistTasksDone,
    /// Tasks were added to a checklist.
    ChecklistTasksAdded,
    /// A chat was added to a community.
    CommunityChatAdded,
    /// A chat was removed from a community.
    CommunityChatRemoved,
    /// A user joined the chat through a community.
    CommunityChatJoined,
    /// A gift was sent.
    Gift,
    /// A unique gift was sent or upgraded.
    UniqueGift,
    /// A managed bot was created.
    ManagedBotCreated,
    /// A payment to a bot succeeded.
    SuccessfulPayment,
    /// A payment was refunded.
    RefundedPayment,
    /// A suggested post was approved.
    SuggestedPostApproved,
    /// A suggested post couldn't be approved because the balance was too low.
    SuggestedPostApprovalFailed,
    /// A suggested post was declined.
    SuggestedPostDeclined,
    /// A suggested post was paid for.
    SuggestedPostPaid,
    /// A suggested post was refunded.
    SuggestedPostRefunded,
    /// A user boosted the chat.
    BoostAdded,
    /// A user set a new high score in a game.
    GameHighScore,
    /// A Mini App sent data to a bot.
    WebAppData,
    /// The chat owner left and a new owner is pending.
    ChatOwnerLeft,
    /// The chat owner changed.
    ChatOwnerChanged,
    /// A contact joined Telegram.
    ContactRegistered,
    /// A custom service message.
    CustomAction,
    /// A Premium gift code was sent.
    PremiumGiftCode,
    /// Telegram Premium was gifted.
    GiftedPremium,
    /// Stars were gifted.
    GiftedStars,
    /// Toncoin was gifted.
    GiftedGrams,
    /// A giveaway was created.
    GiveawayCreated,
    /// A giveaway finished and its winners were chosen.
    GiveawayCompleted,
    /// A giveaway prize of Stars was awarded.
    GiveawayPrizeStars,
    /// The chat history was cleared.
    HistoryCleared,
    /// A phone call started.
    PhoneCallStarted,
    /// A phone call ended. Its service message has a discard reason.
    PhoneCallEnded,
    /// A conference call.
    ConferenceCall,
    /// A screenshot of the chat was taken.
    ScreenshotTaken,
    /// An offer to buy an upgraded gift was made.
    UpgradedGiftPurchaseOffer,
    /// An offer to buy an upgraded gift was declined.
    UpgradedGiftPurchaseOfferRejected,
    /// Protection of the chat's content was switched on or off.
    ChatHasProtectedContentToggled,
    /// A user asked to switch off protection of the chat's content.
    ChatHasProtectedContentDisableRequested,
    /// The chat theme changed.
    ChatSetTheme,
    /// The chat background changed.
    ChatBackgroundSet,
    /// A user suggested a profile photo.
    SuggestProfilePhoto,
    /// A user suggested a birthday.
    SuggestBirthday,
    /// Paid messages were refunded.
    PaidMessagesRefunded,
    /// The price per message changed in a chat that isn't known to be a channel.
    PaidMessagePriceChanged,
    /// The price of direct messages changed in a channel.
    DirectMessagePriceChanged,
    /// An option was added to a poll.
    PollOptionAdded,
    /// An option was removed from a poll.
    PollOptionDeleted,
    /// The user sent Telegram Passport data to a bot.
    PassportDataSent,
    /// A bot received Telegram Passport data.
    PassportDataReceived,
    /// A TON wallet transfer was made.
    TonWalletTransfer,
    /// A TON Connect request was made.
    TonConnectRequest,
    /// A token that lets a user message without paying was used.
    NoPaidMessagesTokenUsed,
    /// A service message this library has no kind for.
    Unknown,
}

impl ServiceKind {
    /// `chat` tells a supergroup's creation from a channel's: both are `messageActionChannelCreate`. It also tells a
    /// channel's direct-message price from a group's message price: both are `messageActionPaidMessagesPrice`.
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
            // The payer's own receipt has no friendly type, so it stays Unknown.
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
            A::MessageActionWebViewDataSentMe(_) | A::MessageActionWebViewDataSent(_) => K::WebAppData,
            A::MessageActionNewCreatorPending(_) => K::ChatOwnerLeft,
            A::MessageActionChangeCreator(_) => K::ChatOwnerChanged,
            A::MessageActionContactSignUp => K::ContactRegistered,
            A::MessageActionCustomAction(_) => K::CustomAction,
            A::MessageActionGiftCode(_) => K::PremiumGiftCode,
            A::MessageActionGiftPremium(_) => K::GiftedPremium,
            A::MessageActionGiftStars(_) => K::GiftedStars,
            A::MessageActionGiftTon(_) => K::GiftedGrams,
            A::MessageActionGiveawayLaunch(_) => K::GiveawayCreated,
            A::MessageActionGiveawayResults(_) => K::GiveawayCompleted,
            A::MessageActionPrizeStars(_) => K::GiveawayPrizeStars,
            A::MessageActionHistoryClear => K::HistoryCleared,
            A::MessageActionPhoneCall(c) if c.reason.is_some() => K::PhoneCallEnded,
            A::MessageActionPhoneCall(_) => K::PhoneCallStarted,
            A::MessageActionConferenceCall(_) => K::ConferenceCall,
            A::MessageActionScreenshotTaken => K::ScreenshotTaken,
            A::MessageActionStarGiftPurchaseOffer(_) => K::UpgradedGiftPurchaseOffer,
            A::MessageActionStarGiftPurchaseOfferDeclined(_) => K::UpgradedGiftPurchaseOfferRejected,
            A::MessageActionNoForwardsToggle(_) => K::ChatHasProtectedContentToggled,
            A::MessageActionNoForwardsRequest(_) => K::ChatHasProtectedContentDisableRequested,
            A::MessageActionSetChatTheme(_) => K::ChatSetTheme,
            A::MessageActionSetChatWallPaper(_) => K::ChatBackgroundSet,
            A::MessageActionSuggestProfilePhoto(_) => K::SuggestProfilePhoto,
            A::MessageActionSuggestBirthday(_) => K::SuggestBirthday,
            A::MessageActionPaidMessagesRefunded(_) => K::PaidMessagesRefunded,
            // A channel's price is for its direct messages; elsewhere it's what each message costs.
            A::MessageActionPaidMessagesPrice(_) if chat == Some(ChatType::Channel) => K::DirectMessagePriceChanged,
            A::MessageActionPaidMessagesPrice(_) => K::PaidMessagePriceChanged,
            A::MessageActionPollAppendAnswer(_) => K::PollOptionAdded,
            A::MessageActionPollDeleteAnswer(_) => K::PollOptionDeleted,
            A::MessageActionSecureValuesSent(_) => K::PassportDataSent,
            A::MessageActionSecureValuesSentMe(_) => K::PassportDataReceived,
            A::MessageActionGramTransfer(_) => K::TonWalletTransfer,
            A::MessageActionWalletTonConnectRequest(_) => K::TonConnectRequest,
            A::MessageActionNoPaidMessagesTokenUsed => K::NoPaidMessagesTokenUsed,
            _ => K::Unknown,
        }
    }
}
