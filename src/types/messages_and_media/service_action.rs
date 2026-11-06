use crate::dispatcher::Entities;
use crate::enums::{PhoneCallDiscardReason, ServiceKind, SuggestedPostRefundReason};
use crate::raw::{base, types};
use crate::types::{
    Birthday, Chat, ChatBackground, ChatShared, ChatTheme, ChecklistTask, ChecklistTasksAdded, ChecklistTasksDone,
    Community, ForumTopicCreated, ForumTopicEdited, GiftInfo, GiftedGrams, GiftedPremium, GiftedStars,
    GiveawayPrizeStars, PhoneCallEnded, Photo, PollOption, PremiumGiftCode, ProximityAlertTriggered, RefundedPayment,
    SharedUser, SuccessfulPayment, SuggestedPostApprovalFailed, SuggestedPostApproved, SuggestedPostDeclined,
    SuggestedPostPaid, SuggestedPostPrice, SuggestedPostRefunded, TonConnectRequest, TonWalletTransfer, UniqueGiftInfo,
    UpgradedGiftPurchaseOffer, UpgradedGiftPurchaseOfferRejected, User, UsersShared, WebAppData, WriteAccessAllowed,
};
use crate::utils::{get_channel_id, get_peer_id};

/// What a service message reports, such as a member joining or a message being pinned.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceAction {
    /// Users who joined or were added. A join by link or request lists the sender alone.
    NewChatMembers(Vec<User>),
    /// A user left or was removed.
    LeftChatMember(User),
    /// The chat's new title.
    NewChatTitle(String),
    /// The chat's new photo.
    NewChatPhoto(Box<Photo>),
    /// The chat's photo was removed.
    DeleteChatPhoto,
    /// A basic group was created.
    GroupChatCreated,
    /// A supergroup was created.
    SupergroupChatCreated,
    /// A channel was created.
    ChannelChatCreated,
    /// The group was upgraded to the supergroup with this chat id.
    MigrateToChatId(i64),
    /// The supergroup was upgraded from the group with this chat id.
    MigrateFromChatId(i64),
    /// A message was pinned. Holds the id of the pinned message.
    PinnedMessage(i32),
    /// The auto-delete timer changed. Holds the new period in seconds, 0 when auto-deletion is off.
    AutoDeleteTimerChanged(i32),
    /// A video chat was scheduled.
    VideoChatScheduled {
        /// When it is due to start, as a Unix timestamp.
        start_date: i32,
    },
    /// A video chat started.
    VideoChatStarted,
    /// A video chat ended.
    VideoChatEnded {
        /// How long the video chat lasted, in seconds.
        duration: i32,
    },
    /// Users were invited to the video chat.
    VideoChatParticipantsInvited(Vec<User>),
    /// The user allowed the bot to write to them.
    WriteAccessAllowed(WriteAccessAllowed),
    /// The domain the user logged in to with Telegram Login.
    ConnectedWebsite(String),
    /// Users the sender picked for a request button.
    UsersShared(UsersShared),
    /// A chat the sender picked for a request button.
    ChatShared(Box<ChatShared>),
    /// A proximity alert fired.
    ProximityAlertTriggered(Box<ProximityAlertTriggered>),
    /// The chat was added to this community.
    CommunityChatAdded(Community),
    /// The chat was removed from its community.
    CommunityChatRemoved,
    /// Someone joined the chat through this community.
    CommunityChatJoined(Community),
    /// A managed bot was created. Holds the bot.
    ManagedBotCreated(User),
    /// A suggested post was approved.
    SuggestedPostApproved(SuggestedPostApproved),
    /// Approving a suggested post failed.
    SuggestedPostApprovalFailed(SuggestedPostApprovalFailed),
    /// A suggested post was declined.
    SuggestedPostDeclined(SuggestedPostDeclined),
    /// The payment for a suggested post went through.
    SuggestedPostPaid(SuggestedPostPaid),
    /// The payment for a suggested post was refunded.
    SuggestedPostRefunded(SuggestedPostRefunded),
    /// A forum topic was created.
    ForumTopicCreated(ForumTopicCreated),
    /// A forum topic's title or icon was changed.
    ForumTopicEdited(ForumTopicEdited),
    /// A forum topic was closed.
    ForumTopicClosed,
    /// A forum topic was reopened.
    ForumTopicReopened,
    /// The General forum topic was hidden.
    GeneralForumTopicHidden,
    /// The General forum topic was shown again.
    GeneralForumTopicUnhidden,
    /// A gift was sent.
    Gift(Box<GiftInfo>),
    /// A unique gift was received, upgraded or transferred.
    UniqueGift(Box<UniqueGiftInfo>),
    /// An invoice was paid.
    SuccessfulPayment(Box<SuccessfulPayment>),
    /// A payment was refunded.
    RefundedPayment(Box<RefundedPayment>),
    /// Checklist tasks were marked done or not done.
    ChecklistTasksDone(ChecklistTasksDone),
    /// Tasks were added to a checklist.
    ChecklistTasksAdded(ChecklistTasksAdded),
    /// A player set a new score in a game.
    GameHighScore {
        /// Id of the game the score was set in.
        game_id: i64,
        /// The player's new score.
        score: i32,
    },
    /// The sender boosted the chat. Holds how many boosts they applied.
    BoostAdded(i32),
    /// Data a web app sent through its button.
    WebAppData(Box<WebAppData>),
    /// The owner left the chat. This user becomes the owner once the transfer completes.
    ChatOwnerLeft(User),
    /// Ownership of the chat passed to this user.
    ChatOwnerChanged(User),
    /// A contact joined Telegram.
    ContactRegistered,
    /// A service message with custom text. Holds the text.
    CustomAction(String),
    /// A Telegram Premium gift code was sent.
    PremiumGiftCode(Box<PremiumGiftCode>),
    /// Telegram Premium was gifted.
    GiftedPremium(Box<GiftedPremium>),
    /// Telegram Stars were gifted.
    GiftedStars(Box<GiftedStars>),
    /// Toncoin was gifted.
    GiftedGrams(Box<GiftedGrams>),
    /// A giveaway was started.
    GiveawayCreated {
        /// Stars to split among the winners. `None` unless the giveaway is of Telegram Stars.
        prize_star_count: Option<i64>,
    },
    /// A giveaway ended. The giveaway message is the reply target.
    GiveawayCompleted {
        /// Number of winners.
        winner_count: i32,
        /// Number of prizes nobody claimed.
        unclaimed_prize_count: i32,
        /// Whether the prizes are Telegram Stars.
        is_star_giveaway: bool,
    },
    /// Stars won in a giveaway.
    GiveawayPrizeStars(Box<GiveawayPrizeStars>),
    /// The chat history was cleared.
    HistoryCleared,
    /// A phone call started.
    PhoneCallStarted {
        /// Id of the call.
        id: i64,
        /// `true` for a video call.
        is_video: bool,
    },
    /// A phone call ended.
    PhoneCallEnded(Box<PhoneCallEnded>),
    /// A conference call took place.
    ConferenceCall {
        /// Id of the call.
        id: i64,
        /// Whether the call was missed.
        is_missed: bool,
        /// Whether the call is still going.
        is_active: bool,
        /// `true` for a video call.
        is_video: bool,
        /// How long the call lasted, in seconds.
        duration: Option<i32>,
    },
    /// A screenshot of the chat was taken.
    ScreenshotTaken,
    /// Someone offered to buy an upgraded gift.
    UpgradedGiftPurchaseOffer(Box<UpgradedGiftPurchaseOffer>),
    /// An offer to buy an upgraded gift was rejected.
    UpgradedGiftPurchaseOfferRejected(Box<UpgradedGiftPurchaseOfferRejected>),
    /// Protected content was turned on or off in the chat.
    ChatHasProtectedContentToggled {
        /// Whether content was protected before.
        old_has_protected_content: bool,
        /// Whether content is protected now.
        new_has_protected_content: bool,
    },
    /// A request to turn off protected content in a private chat.
    ChatHasProtectedContentDisableRequested {
        /// Whether the request has expired.
        is_expired: bool,
    },
    /// The chat's theme was changed.
    ChatSetTheme(Box<ChatTheme>),
    /// The chat's background was changed.
    ChatBackgroundSet {
        /// The new background.
        background: Box<ChatBackground>,
        /// `true` if the sender applied a background the other user had sent earlier.
        is_same: bool,
        /// `true` if the background was set for both users of the private chat.
        is_for_both: bool,
    },
    /// The sender suggests this photo as the recipient's profile photo.
    SuggestProfilePhoto(Box<Photo>),
    /// The sender suggests this birthday for the recipient's profile.
    SuggestBirthday(Birthday),
    /// Paid messages were refunded.
    PaidMessagesRefunded {
        /// Number of messages refunded.
        message_count: i32,
        /// Total Stars refunded.
        star_count: i64,
    },
    /// The price of a message to this chat changed.
    PaidMessagePriceChanged {
        /// Stars a message now costs.
        paid_message_star_count: i64,
    },
    /// The price of direct messages to the channel changed.
    DirectMessagePriceChanged {
        /// Whether direct messages to the channel are allowed.
        is_enabled: bool,
        /// Stars a direct message now costs.
        paid_message_star_count: i64,
    },
    /// An option was added to a poll. Holds the new option. The poll is the reply target.
    PollOptionAdded(Box<PollOption>),
    /// An option was removed from a poll. Holds the removed option. The poll is the reply target.
    PollOptionDeleted(Box<PollOption>),
    /// The user sent Telegram Passport data to a bot.
    PassportDataSent,
    /// The bot received Telegram Passport data. The data stays encrypted.
    PassportDataReceived,
    /// Toncoin was transferred through a TON wallet.
    TonWalletTransfer(Box<TonWalletTransfer>),
    /// A TON Connect request.
    TonConnectRequest(Box<TonConnectRequest>),
    /// Someone used a no-paid-messages token to write without paying.
    NoPaidMessagesTokenUsed,
    /// A service message of a kind this crate has no variant for, the action [`ServiceKind::Unknown`] names.
    #[non_exhaustive]
    Unknown {
        /// TL constructor id of the message action, for logging.
        constructor_id: u32,
    },
}

impl ServiceAction {
    /// The variant `ServiceKind::of` names, so `filters::service_kind` and `service` always agree. `None` only for a
    /// known kind whose payload is malformed (a pin without its header).
    pub(crate) fn parse(raw: &types::MessageService, entities: &Entities) -> Option<ServiceAction> {
        use ServiceKind as K;
        use base::MessageAction as A;
        let chat = Chat::kind_of(&raw.peer_id, entities, raw.post);
        let users = |ids: &[i64]| ids.iter().map(|&id| User::parse(id, entities)).collect();
        Some(match (ServiceKind::of(&raw.action, Some(chat)), &raw.action) {
            (K::NewChatMembers, A::MessageActionChatAddUser(a)) => ServiceAction::NewChatMembers(users(&a.users)),
            // Whoever joins by link or request is the sender.
            (K::NewChatMembers, _) => match &raw.from_id {
                Some(base::Peer::PeerUser(p)) => ServiceAction::NewChatMembers(vec![User::parse(p.user_id, entities)]),
                _ => return None,
            },
            (K::LeftChatMember, A::MessageActionChatDeleteUser(a)) => {
                ServiceAction::LeftChatMember(User::parse(a.user_id, entities))
            }
            (K::NewChatTitle, A::MessageActionChatEditTitle(a)) => ServiceAction::NewChatTitle(a.title.clone()),
            (K::NewChatPhoto, A::MessageActionChatEditPhoto(a)) => {
                ServiceAction::NewChatPhoto(Box::new(Photo::parse(&a.photo)?))
            }
            (K::DeleteChatPhoto, _) => ServiceAction::DeleteChatPhoto,
            (K::GroupChatCreated, _) => ServiceAction::GroupChatCreated,
            (K::SupergroupChatCreated, _) => ServiceAction::SupergroupChatCreated,
            (K::ChannelChatCreated, _) => ServiceAction::ChannelChatCreated,
            (K::MigrateToChatId, A::MessageActionChatMigrateTo(a)) => {
                ServiceAction::MigrateToChatId(get_channel_id(a.channel_id))
            }
            (K::MigrateFromChatId, A::MessageActionChannelMigrateFrom(a)) => {
                ServiceAction::MigrateFromChatId(-a.chat_id)
            }
            // The reply header points at the pinned message.
            (K::PinnedMessage, _) => match &raw.reply_to {
                Some(base::MessageReplyHeader::MessageReplyHeader(r)) => {
                    ServiceAction::PinnedMessage(r.reply_to_msg_id?)
                }
                _ => return None,
            },
            (K::AutoDeleteTimerChanged, A::MessageActionSetMessagesTTL(a)) => {
                ServiceAction::AutoDeleteTimerChanged(a.period)
            }
            (K::VideoChatScheduled, A::MessageActionGroupCallScheduled(a)) => {
                ServiceAction::VideoChatScheduled { start_date: a.schedule_date }
            }
            (K::VideoChatStarted, _) => ServiceAction::VideoChatStarted,
            (K::VideoChatEnded, A::MessageActionGroupCall(a)) => {
                ServiceAction::VideoChatEnded { duration: a.duration? }
            }
            (K::VideoChatParticipantsInvited, A::MessageActionInviteToGroupCall(a)) => {
                ServiceAction::VideoChatParticipantsInvited(users(&a.users))
            }
            (K::WriteAccessAllowed, A::MessageActionBotAllowed(a)) => {
                ServiceAction::WriteAccessAllowed(WriteAccessAllowed {
                    from_request: a.from_request,
                    web_app_name: match &a.app {
                        Some(base::BotApp::BotApp(app)) => Some(app.short_name.clone()),
                        _ => None,
                    },
                    from_attachment_menu: a.attach_menu,
                })
            }
            (K::ConnectedWebsite, A::MessageActionBotAllowed(a)) => ServiceAction::ConnectedWebsite(a.domain.clone()?),
            (K::UsersShared, A::MessageActionRequestedPeerSentMe(a)) => {
                let users = a.peers.iter().filter_map(shared_user).collect();
                ServiceAction::UsersShared(UsersShared { request_id: a.button_id, users })
            }
            (K::ChatShared, A::MessageActionRequestedPeerSentMe(a)) => {
                ServiceAction::ChatShared(Box::new(a.peers.iter().find_map(|p| shared_chat(a.button_id, p))?))
            }
            // The requesting user's own copy of the same message only carries peer ids.
            (K::UsersShared, A::MessageActionRequestedPeer(a)) => {
                let users = a.peers.iter().map(|p| SharedUser { user_id: get_peer_id(p), ..Default::default() });
                ServiceAction::UsersShared(UsersShared { request_id: a.button_id, users: users.collect() })
            }
            (K::ChatShared, A::MessageActionRequestedPeer(a)) => {
                let peer = a.peers.iter().find(|p| !matches!(p, base::Peer::PeerUser(_)))?;
                let chat = ChatShared { request_id: a.button_id, chat_id: get_peer_id(peer), ..Default::default() };
                ServiceAction::ChatShared(Box::new(chat))
            }
            (K::ProximityAlertTriggered, A::MessageActionGeoProximityReached(a)) => {
                ServiceAction::ProximityAlertTriggered(Box::new(ProximityAlertTriggered {
                    traveler: User::parse(get_peer_id(&a.from_id), entities),
                    watcher: User::parse(get_peer_id(&a.to_id), entities),
                    distance: a.distance,
                }))
            }
            (K::CommunityChatAdded, A::MessageActionChangeCommunity(a)) => {
                ServiceAction::CommunityChatAdded(Community::parse(a.community_id?, entities))
            }
            (K::CommunityChatRemoved, _) => ServiceAction::CommunityChatRemoved,
            (K::CommunityChatJoined, A::MessageActionChatJoinedViaCommunity(a)) => {
                ServiceAction::CommunityChatJoined(Community::parse(a.community_id, entities))
            }
            (K::ManagedBotCreated, A::MessageActionManagedBotCreated(a)) => {
                ServiceAction::ManagedBotCreated(User::parse(a.bot_id, entities))
            }
            (K::SuggestedPostApproved, A::MessageActionSuggestedPostApproval(a)) => {
                let price = a.price.as_ref().map(SuggestedPostPrice::parse);
                ServiceAction::SuggestedPostApproved(SuggestedPostApproved { price, send_date: a.schedule_date })
            }
            (K::SuggestedPostApprovalFailed, A::MessageActionSuggestedPostApproval(a)) => {
                let price = a.price.as_ref().map(SuggestedPostPrice::parse);
                ServiceAction::SuggestedPostApprovalFailed(SuggestedPostApprovalFailed { price })
            }
            (K::SuggestedPostDeclined, A::MessageActionSuggestedPostApproval(a)) => {
                ServiceAction::SuggestedPostDeclined(SuggestedPostDeclined { comment: a.reject_comment.clone() })
            }
            (K::SuggestedPostPaid, A::MessageActionSuggestedPostSuccess(a)) => {
                ServiceAction::SuggestedPostPaid(SuggestedPostPaid { price: SuggestedPostPrice::parse(&a.price) })
            }
            (K::SuggestedPostRefunded, A::MessageActionSuggestedPostRefund(a)) => {
                let reason = if a.payer_initiated {
                    SuggestedPostRefundReason::PaymentRefunded
                } else {
                    SuggestedPostRefundReason::PostDeleted
                };
                ServiceAction::SuggestedPostRefunded(SuggestedPostRefunded { reason })
            }
            (K::ForumTopicCreated, A::MessageActionTopicCreate(a)) => {
                ServiceAction::ForumTopicCreated(ForumTopicCreated {
                    title: a.title.clone(),
                    icon_color: a.icon_color,
                    icon_emoji_id: a.icon_emoji_id,
                    is_name_implicit: a.title_missing,
                })
            }
            (K::ForumTopicEdited, A::MessageActionTopicEdit(a)) => ServiceAction::ForumTopicEdited(ForumTopicEdited {
                title: a.title.clone(),
                icon_emoji_id: a.icon_emoji_id,
            }),
            (K::ForumTopicClosed, _) => ServiceAction::ForumTopicClosed,
            (K::ForumTopicReopened, _) => ServiceAction::ForumTopicReopened,
            (K::GeneralForumTopicHidden, _) => ServiceAction::GeneralForumTopicHidden,
            (K::GeneralForumTopicUnhidden, _) => ServiceAction::GeneralForumTopicUnhidden,
            // The service message's own id addresses a gift a user keeps.
            (K::Gift, A::MessageActionStarGift(a)) => {
                ServiceAction::Gift(Box::new(GiftInfo::parse(a, raw.id, entities)?))
            }
            (K::UniqueGift, A::MessageActionStarGiftUnique(a)) => {
                ServiceAction::UniqueGift(Box::new(UniqueGiftInfo::parse(a, raw.id, entities)?))
            }
            (K::SuccessfulPayment, A::MessageActionPaymentSentMe(a)) => {
                ServiceAction::SuccessfulPayment(Box::new(SuccessfulPayment::parse(a)))
            }
            (K::RefundedPayment, A::MessageActionPaymentRefunded(a)) => {
                ServiceAction::RefundedPayment(Box::new(RefundedPayment::parse(a)))
            }
            (K::ChecklistTasksDone, A::MessageActionTodoCompletions(a)) => {
                ServiceAction::ChecklistTasksDone(ChecklistTasksDone {
                    marked_as_done_task_ids: a.completed.clone(),
                    marked_as_not_done_task_ids: a.incompleted.clone(),
                })
            }
            (K::ChecklistTasksAdded, A::MessageActionTodoAppendTasks(a)) => {
                ServiceAction::ChecklistTasksAdded(ChecklistTasksAdded {
                    tasks: a.list.iter().map(|item| ChecklistTask::parse(item, entities)).collect(),
                })
            }
            (K::GameHighScore, A::MessageActionGameScore(a)) => {
                ServiceAction::GameHighScore { game_id: a.game_id, score: a.score }
            }
            (K::BoostAdded, A::MessageActionBoostApply(a)) => ServiceAction::BoostAdded(a.boosts),
            (K::WebAppData, A::MessageActionWebViewDataSentMe(a)) => ServiceAction::WebAppData(Box::new(WebAppData {
                button_text: a.text.clone(),
                data: Some(a.data.clone()),
            })),
            (K::WebAppData, A::MessageActionWebViewDataSent(a)) => {
                ServiceAction::WebAppData(Box::new(WebAppData { button_text: a.text.clone(), data: None }))
            }
            (K::ChatOwnerLeft, A::MessageActionNewCreatorPending(a)) => {
                ServiceAction::ChatOwnerLeft(User::parse(a.new_creator_id, entities))
            }
            (K::ChatOwnerChanged, A::MessageActionChangeCreator(a)) => {
                ServiceAction::ChatOwnerChanged(User::parse(a.new_creator_id, entities))
            }
            (K::ContactRegistered, _) => ServiceAction::ContactRegistered,
            (K::CustomAction, A::MessageActionCustomAction(a)) => ServiceAction::CustomAction(a.message.clone()),
            (K::PremiumGiftCode, A::MessageActionGiftCode(a)) => {
                ServiceAction::PremiumGiftCode(Box::new(PremiumGiftCode::parse(a, entities)))
            }
            (K::GiftedPremium, A::MessageActionGiftPremium(a)) => {
                ServiceAction::GiftedPremium(Box::new(GiftedPremium::parse(a, entities)))
            }
            (K::GiftedStars, A::MessageActionGiftStars(a)) => {
                ServiceAction::GiftedStars(Box::new(GiftedStars::parse(a)))
            }
            (K::GiftedGrams, A::MessageActionGiftTon(a)) => ServiceAction::GiftedGrams(Box::new(GiftedGrams::parse(a))),
            (K::GiveawayCreated, A::MessageActionGiveawayLaunch(a)) => {
                ServiceAction::GiveawayCreated { prize_star_count: a.stars }
            }
            (K::GiveawayCompleted, A::MessageActionGiveawayResults(a)) => ServiceAction::GiveawayCompleted {
                winner_count: a.winners_count,
                unclaimed_prize_count: a.unclaimed_count,
                is_star_giveaway: a.stars,
            },
            (K::GiveawayPrizeStars, A::MessageActionPrizeStars(a)) => {
                ServiceAction::GiveawayPrizeStars(Box::new(GiveawayPrizeStars::parse(a, entities)))
            }
            (K::HistoryCleared, _) => ServiceAction::HistoryCleared,
            (K::PhoneCallStarted, A::MessageActionPhoneCall(a)) => {
                ServiceAction::PhoneCallStarted { id: a.call_id, is_video: a.video }
            }
            (K::PhoneCallEnded, A::MessageActionPhoneCall(a)) => {
                ServiceAction::PhoneCallEnded(Box::new(PhoneCallEnded {
                    id: a.call_id,
                    is_video: a.video,
                    reason: PhoneCallDiscardReason::parse(a.reason.as_ref()?),
                    duration: a.duration,
                }))
            }
            (K::ConferenceCall, A::MessageActionConferenceCall(a)) => ServiceAction::ConferenceCall {
                id: a.call_id,
                is_missed: a.missed,
                is_active: a.active,
                is_video: a.video,
                duration: a.duration,
            },
            (K::ScreenshotTaken, _) => ServiceAction::ScreenshotTaken,
            (K::UpgradedGiftPurchaseOffer, A::MessageActionStarGiftPurchaseOffer(a)) => {
                ServiceAction::UpgradedGiftPurchaseOffer(Box::new(UpgradedGiftPurchaseOffer::parse(a, entities)?))
            }
            (K::UpgradedGiftPurchaseOfferRejected, A::MessageActionStarGiftPurchaseOfferDeclined(a)) => {
                let rejected = UpgradedGiftPurchaseOfferRejected::parse(a, entities)?;
                ServiceAction::UpgradedGiftPurchaseOfferRejected(Box::new(rejected))
            }
            (K::ChatHasProtectedContentToggled, A::MessageActionNoForwardsToggle(a)) => {
                ServiceAction::ChatHasProtectedContentToggled {
                    old_has_protected_content: a.prev_value,
                    new_has_protected_content: a.new_value,
                }
            }
            (K::ChatHasProtectedContentDisableRequested, A::MessageActionNoForwardsRequest(a)) => {
                ServiceAction::ChatHasProtectedContentDisableRequested { is_expired: a.expired }
            }
            (K::ChatSetTheme, A::MessageActionSetChatTheme(a)) => {
                ServiceAction::ChatSetTheme(Box::new(ChatTheme::parse(&a.theme, entities)))
            }
            (K::ChatBackgroundSet, A::MessageActionSetChatWallPaper(a)) => ServiceAction::ChatBackgroundSet {
                background: Box::new(ChatBackground::parse(a.wallpaper.clone())),
                is_same: a.same,
                is_for_both: a.for_both,
            },
            (K::SuggestProfilePhoto, A::MessageActionSuggestProfilePhoto(a)) => {
                ServiceAction::SuggestProfilePhoto(Box::new(Photo::parse(&a.photo)?))
            }
            (K::SuggestBirthday, A::MessageActionSuggestBirthday(a)) => {
                ServiceAction::SuggestBirthday(Birthday::parse(&a.birthday))
            }
            (K::PaidMessagesRefunded, A::MessageActionPaidMessagesRefunded(a)) => {
                ServiceAction::PaidMessagesRefunded { message_count: a.count, star_count: a.stars }
            }
            (K::PaidMessagePriceChanged, A::MessageActionPaidMessagesPrice(a)) => {
                ServiceAction::PaidMessagePriceChanged { paid_message_star_count: a.stars }
            }
            (K::DirectMessagePriceChanged, A::MessageActionPaidMessagesPrice(a)) => {
                ServiceAction::DirectMessagePriceChanged {
                    is_enabled: a.broadcast_messages_allowed,
                    paid_message_star_count: a.stars,
                }
            }
            (K::PollOptionAdded, A::MessageActionPollAppendAnswer(a)) => {
                ServiceAction::PollOptionAdded(Box::new(PollOption::from_answer(&a.answer, entities)))
            }
            (K::PollOptionDeleted, A::MessageActionPollDeleteAnswer(a)) => {
                ServiceAction::PollOptionDeleted(Box::new(PollOption::from_answer(&a.answer, entities)))
            }
            (K::PassportDataSent, _) => ServiceAction::PassportDataSent,
            (K::PassportDataReceived, _) => ServiceAction::PassportDataReceived,
            (K::TonWalletTransfer, A::MessageActionGramTransfer(a)) => ServiceAction::TonWalletTransfer(Box::new(
                TonWalletTransfer::parse(a, raw.from_id.as_ref(), &raw.peer_id, entities),
            )),
            (K::TonConnectRequest, A::MessageActionWalletTonConnectRequest(a)) => {
                ServiceAction::TonConnectRequest(Box::new(TonConnectRequest::parse(a)))
            }
            (K::NoPaidMessagesTokenUsed, _) => ServiceAction::NoPaidMessagesTokenUsed,
            (K::Unknown, action) => ServiceAction::Unknown { constructor_id: action.constructor_id() },
            _ => return None,
        })
    }

    pub(crate) fn kind(&self) -> ServiceKind {
        use ServiceAction as S;
        use ServiceKind as K;
        match self {
            S::NewChatMembers(_) => K::NewChatMembers,
            S::LeftChatMember(_) => K::LeftChatMember,
            S::NewChatTitle(_) => K::NewChatTitle,
            S::NewChatPhoto(_) => K::NewChatPhoto,
            S::DeleteChatPhoto => K::DeleteChatPhoto,
            S::GroupChatCreated => K::GroupChatCreated,
            S::SupergroupChatCreated => K::SupergroupChatCreated,
            S::ChannelChatCreated => K::ChannelChatCreated,
            S::MigrateToChatId(_) => K::MigrateToChatId,
            S::MigrateFromChatId(_) => K::MigrateFromChatId,
            S::PinnedMessage(_) => K::PinnedMessage,
            S::AutoDeleteTimerChanged(_) => K::AutoDeleteTimerChanged,
            S::VideoChatScheduled { .. } => K::VideoChatScheduled,
            S::VideoChatStarted => K::VideoChatStarted,
            S::VideoChatEnded { .. } => K::VideoChatEnded,
            S::VideoChatParticipantsInvited(_) => K::VideoChatParticipantsInvited,
            S::WriteAccessAllowed(_) => K::WriteAccessAllowed,
            S::ConnectedWebsite(_) => K::ConnectedWebsite,
            S::UsersShared(_) => K::UsersShared,
            S::ChatShared(_) => K::ChatShared,
            S::ProximityAlertTriggered(_) => K::ProximityAlertTriggered,
            S::CommunityChatAdded(_) => K::CommunityChatAdded,
            S::CommunityChatRemoved => K::CommunityChatRemoved,
            S::CommunityChatJoined(_) => K::CommunityChatJoined,
            S::ManagedBotCreated(_) => K::ManagedBotCreated,
            S::SuggestedPostApproved(_) => K::SuggestedPostApproved,
            S::SuggestedPostApprovalFailed(_) => K::SuggestedPostApprovalFailed,
            S::SuggestedPostDeclined(_) => K::SuggestedPostDeclined,
            S::SuggestedPostPaid(_) => K::SuggestedPostPaid,
            S::SuggestedPostRefunded(_) => K::SuggestedPostRefunded,
            S::ForumTopicCreated(_) => K::ForumTopicCreated,
            S::ForumTopicEdited(_) => K::ForumTopicEdited,
            S::ForumTopicClosed => K::ForumTopicClosed,
            S::ForumTopicReopened => K::ForumTopicReopened,
            S::GeneralForumTopicHidden => K::GeneralForumTopicHidden,
            S::GeneralForumTopicUnhidden => K::GeneralForumTopicUnhidden,
            S::Gift(_) => K::Gift,
            S::UniqueGift(_) => K::UniqueGift,
            S::SuccessfulPayment(_) => K::SuccessfulPayment,
            S::RefundedPayment(_) => K::RefundedPayment,
            S::ChecklistTasksDone(_) => K::ChecklistTasksDone,
            S::ChecklistTasksAdded(_) => K::ChecklistTasksAdded,
            S::GameHighScore { .. } => K::GameHighScore,
            S::BoostAdded(_) => K::BoostAdded,
            S::WebAppData(_) => K::WebAppData,
            S::ChatOwnerLeft(_) => K::ChatOwnerLeft,
            S::ChatOwnerChanged(_) => K::ChatOwnerChanged,
            S::ContactRegistered => K::ContactRegistered,
            S::CustomAction(_) => K::CustomAction,
            S::PremiumGiftCode(_) => K::PremiumGiftCode,
            S::GiftedPremium(_) => K::GiftedPremium,
            S::GiftedStars(_) => K::GiftedStars,
            S::GiftedGrams(_) => K::GiftedGrams,
            S::GiveawayCreated { .. } => K::GiveawayCreated,
            S::GiveawayCompleted { .. } => K::GiveawayCompleted,
            S::GiveawayPrizeStars(_) => K::GiveawayPrizeStars,
            S::HistoryCleared => K::HistoryCleared,
            S::PhoneCallStarted { .. } => K::PhoneCallStarted,
            S::PhoneCallEnded(_) => K::PhoneCallEnded,
            S::ConferenceCall { .. } => K::ConferenceCall,
            S::ScreenshotTaken => K::ScreenshotTaken,
            S::UpgradedGiftPurchaseOffer(_) => K::UpgradedGiftPurchaseOffer,
            S::UpgradedGiftPurchaseOfferRejected(_) => K::UpgradedGiftPurchaseOfferRejected,
            S::ChatHasProtectedContentToggled { .. } => K::ChatHasProtectedContentToggled,
            S::ChatHasProtectedContentDisableRequested { .. } => K::ChatHasProtectedContentDisableRequested,
            S::ChatSetTheme(_) => K::ChatSetTheme,
            S::ChatBackgroundSet { .. } => K::ChatBackgroundSet,
            S::SuggestProfilePhoto(_) => K::SuggestProfilePhoto,
            S::SuggestBirthday(_) => K::SuggestBirthday,
            S::PaidMessagesRefunded { .. } => K::PaidMessagesRefunded,
            S::PaidMessagePriceChanged { .. } => K::PaidMessagePriceChanged,
            S::DirectMessagePriceChanged { .. } => K::DirectMessagePriceChanged,
            S::PollOptionAdded(_) => K::PollOptionAdded,
            S::PollOptionDeleted(_) => K::PollOptionDeleted,
            S::PassportDataSent => K::PassportDataSent,
            S::PassportDataReceived => K::PassportDataReceived,
            S::TonWalletTransfer(_) => K::TonWalletTransfer,
            S::TonConnectRequest(_) => K::TonConnectRequest,
            S::NoPaidMessagesTokenUsed => K::NoPaidMessagesTokenUsed,
            S::Unknown { .. } => K::Unknown,
        }
    }
}

fn shared_user(peer: &base::RequestedPeer) -> Option<SharedUser> {
    let base::RequestedPeer::RequestedPeerUser(p) = peer else { return None };
    Some(SharedUser {
        user_id: p.user_id,
        first_name: p.first_name.clone(),
        last_name: p.last_name.clone(),
        username: p.username.clone(),
        photo: p.photo.as_ref().and_then(Photo::parse),
    })
}

fn shared_chat(request_id: i32, peer: &base::RequestedPeer) -> Option<ChatShared> {
    let (chat_id, title, username, photo) = match peer {
        base::RequestedPeer::RequestedPeerUser(_) => return None,
        base::RequestedPeer::RequestedPeerChat(p) => (-p.chat_id, &p.title, None, &p.photo),
        base::RequestedPeer::RequestedPeerChannel(p) => {
            (get_channel_id(p.channel_id), &p.title, p.username.clone(), &p.photo)
        }
    };
    let photo = photo.as_ref().and_then(Photo::parse);
    Some(ChatShared { request_id, chat_id, title: title.clone(), username, photo })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::super::checklist::tests::item;
    use super::super::media::tests::document_media;
    use super::ServiceAction;
    use crate::dispatcher::Entities;
    use crate::enums::{ServiceKind, SuggestedPostRefundReason};
    use crate::raw::{base, types};
    use crate::types::{
        Chat, ChecklistTask, ChecklistTasksAdded, ChecklistTasksDone, SuggestedPostApproved, SuggestedPostPrice,
    };
    use crate::utils::get_peer;

    fn service(action: base::MessageAction) -> types::MessageService {
        types::MessageService {
            out: false,
            mentioned: false,
            media_unread: false,
            reactions_are_possible: false,
            silent: false,
            post: false,
            legacy: false,
            id: 1,
            from_id: Some(get_peer(9)),
            peer_id: get_peer(-1000000000005),
            saved_peer_id: None,
            reply_to: None,
            date: 1,
            action,
            reactions: None,
            ttl_period: None,
        }
    }

    fn parse(action: base::MessageAction) -> Option<ServiceAction> {
        ServiceAction::parse(&service(action), &Entities::default())
    }

    fn reply_to(reply_to_msg_id: i32) -> Option<base::MessageReplyHeader> {
        Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(types::MessageReplyHeader {
            reply_to_scheduled: false,
            forum_topic: false,
            quote: false,
            reply_to_ephemeral: false,
            reply_to_msg_id: Some(reply_to_msg_id),
            reply_to_peer_id: None,
            reply_from: None,
            reply_media: None,
            reply_to_top_id: None,
            quote_text: None,
            quote_entities: None,
            quote_offset: None,
            todo_item_id: None,
            poll_option: None,
            quote_rich: None,
        })))
    }

    fn approval(rejected: bool, balance_too_low: bool) -> base::MessageAction {
        base::MessageAction::MessageActionSuggestedPostApproval(Box::new(types::MessageActionSuggestedPostApproval {
            rejected,
            balance_too_low,
            reject_comment: rejected.then(|| "no".into()),
            schedule_date: Some(9),
            price: Some(base::StarsAmount::StarsTonAmount(types::StarsTonAmount { amount: 5 })),
        }))
    }

    fn topic_edit(title: Option<&str>, closed: Option<bool>, hidden: Option<bool>) -> base::MessageAction {
        let edit = types::MessageActionTopicEdit { title: title.map(Into::into), icon_emoji_id: None, closed, hidden };
        base::MessageAction::MessageActionTopicEdit(Box::new(edit))
    }

    fn charge() -> base::PaymentCharge {
        base::PaymentCharge::PaymentCharge(Box::new(types::PaymentCharge {
            id: "c".into(),
            provider_charge_id: "p".into(),
        }))
    }

    fn star_gift() -> types::MessageActionStarGift {
        let sticker = base::DocumentAttribute::DocumentAttributeSticker(Box::new(types::DocumentAttributeSticker {
            mask: false,
            alt: String::new(),
            stickerset: base::InputStickerSet::InputStickerSetEmpty,
            mask_coords: None,
        }));
        let base::MessageMedia::MessageMediaDocument(media) = document_media("image/webp", vec![sticker]) else {
            panic!("expected a document")
        };
        let gift = types::StarGift {
            limited: false,
            sold_out: false,
            birthday: false,
            require_premium: false,
            limited_per_user: false,
            peer_color_available: false,
            auction: false,
            id: 1,
            sticker: media.document.expect("a sticker"),
            stars: 50,
            availability_remains: None,
            availability_total: None,
            availability_resale: None,
            convert_stars: 40,
            first_sale_date: None,
            last_sale_date: None,
            upgrade_stars: None,
            resell_min_stars: None,
            title: None,
            released_by: None,
            per_user_total: None,
            per_user_remains: None,
            locked_until_date: None,
            auction_slug: None,
            gifts_per_round: None,
            auction_start_date: None,
            upgrade_variants: None,
            background: None,
        };
        types::MessageActionStarGift {
            name_hidden: false,
            saved: false,
            converted: false,
            upgraded: false,
            refunded: false,
            can_upgrade: false,
            prepaid_upgrade: false,
            upgrade_separate: false,
            auction_acquired: false,
            gift: base::StarGift::StarGift(Box::new(gift)),
            message: None,
            convert_stars: None,
            upgrade_msg_id: None,
            upgrade_stars: None,
            from_id: None,
            peer: None,
            saved_id: None,
            prepaid_upgrade_hash: None,
            gift_msg_id: None,
            to_id: None,
            gift_num: None,
        }
    }

    fn unique_gift() -> types::MessageActionStarGiftUnique {
        let gift = types::StarGiftUnique {
            require_premium: false,
            resale_ton_only: false,
            theme_available: false,
            burned: false,
            crafted: false,
            id: 1,
            gift_id: 2,
            title: "Plush Pepe".into(),
            slug: "PlushPepe-3".into(),
            num: 3,
            owner_id: None,
            owner_name: None,
            owner_address: None,
            attributes: Vec::new(),
            availability_issued: 10,
            availability_total: 100,
            gift_address: None,
            resell_amount: None,
            released_by: None,
            value_amount: None,
            value_currency: None,
            value_usd_amount: None,
            theme_peer: None,
            peer_color: None,
            host_id: None,
            offer_min_stars: None,
            craft_chance_permille: None,
        };
        types::MessageActionStarGiftUnique {
            upgrade: false,
            transferred: false,
            saved: false,
            refunded: false,
            prepaid_upgrade: false,
            assigned: false,
            from_offer: false,
            craft: false,
            name_hidden: false,
            gift: base::StarGift::StarGiftUnique(Box::new(gift)),
            can_export_at: None,
            transfer_stars: None,
            from_id: None,
            peer: None,
            saved_id: None,
            resale_amount: None,
            can_transfer_at: None,
            can_resell_at: None,
            drop_original_details_stars: None,
            can_craft_at: None,
            message: None,
        }
    }

    fn stars(amount: i64) -> base::StarsAmount {
        base::StarsAmount::StarsAmount(Box::new(types::StarsAmount { amount, nanos: 0 }))
    }

    fn premium() -> types::MessageActionGiftPremium {
        types::MessageActionGiftPremium {
            currency: "USD".into(),
            amount: 5,
            days: 30,
            crypto_currency: None,
            crypto_amount: None,
            message: Some(base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities {
                text: "enjoy".into(),
                entities: Vec::new(),
            }))),
        }
    }

    fn gift_code() -> types::MessageActionGiftCode {
        types::MessageActionGiftCode {
            via_giveaway: true,
            unclaimed: false,
            boost_peer: Some(get_peer(-1000000000005)),
            days: 30,
            slug: "code".into(),
            currency: None,
            amount: None,
            crypto_currency: None,
            crypto_amount: None,
            message: None,
        }
    }

    fn phone_call(reason: Option<base::PhoneCallDiscardReason>, duration: Option<i32>) -> base::MessageAction {
        base::MessageAction::MessageActionPhoneCall(Box::new(types::MessageActionPhoneCall {
            video: true,
            call_id: 8,
            reason,
            duration,
        }))
    }

    fn offer(gift: base::StarGift, accepted: bool, declined: bool) -> base::MessageAction {
        base::MessageAction::MessageActionStarGiftPurchaseOffer(Box::new(types::MessageActionStarGiftPurchaseOffer {
            accepted,
            declined,
            gift,
            price: stars(5),
            expires_at: 9,
        }))
    }

    fn theme(theme: base::ChatTheme) -> base::MessageAction {
        base::MessageAction::MessageActionSetChatTheme(Box::new(types::MessageActionSetChatTheme { theme }))
    }

    fn wallpaper(same: bool, for_both: bool) -> base::MessageAction {
        let wallpaper = base::WallPaper::WallPaperNoFile(Box::new(types::WallPaperNoFile {
            id: 1,
            default: false,
            dark: true,
            settings: None,
        }));
        base::MessageAction::MessageActionSetChatWallPaper(Box::new(types::MessageActionSetChatWallPaper {
            same,
            for_both,
            wallpaper,
        }))
    }

    fn paid_price(broadcast_messages_allowed: bool) -> base::MessageAction {
        base::MessageAction::MessageActionPaidMessagesPrice(Box::new(types::MessageActionPaidMessagesPrice {
            broadcast_messages_allowed,
            stars: 10,
        }))
    }

    fn poll_answer() -> base::PollAnswer {
        base::PollAnswer::PollAnswer(Box::new(types::PollAnswer {
            text: base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities {
                text: "Tea".into(),
                entities: Vec::new(),
            })),
            option: vec![2],
            media: None,
            added_by: None,
            date: None,
        }))
    }

    // One well-formed message per kind; `filters::service_kind` reads only the raw action, so a filter matches
    // exactly when `service` holds the variant of the same name.
    fn every_kind() -> Vec<types::MessageService> {
        use base::MessageAction as A;
        let call = || base::InputGroupCall::InputGroupCall(Box::new(types::InputGroupCall { id: 1, access_hash: 2 }));
        let photo = base::Photo::Photo(Box::new(types::Photo {
            has_stickers: false,
            id: 1,
            access_hash: 2,
            file_reference: Vec::new(),
            date: 3,
            sizes: vec![base::PhotoSize::PhotoSize(Box::new(types::PhotoSize {
                r#type: "x".into(),
                w: 8,
                h: 8,
                size: 64,
            }))],
            video_sizes: None,
            dc_id: 2,
        }));
        let channel = types::RequestedPeerChannel { channel_id: 3, title: None, username: None, photo: None };
        let user =
            types::RequestedPeerUser { user_id: 7, first_name: None, last_name: None, username: None, photo: None };
        let bot_allowed = |domain: Option<&str>| {
            service(A::MessageActionBotAllowed(Box::new(types::MessageActionBotAllowed {
                attach_menu: true,
                from_request: false,
                domain: domain.map(Into::into),
                app: None,
            })))
        };
        let channel_create =
            || A::MessageActionChannelCreate(Box::new(types::MessageActionChannelCreate { title: "T".into() }));
        vec![
            service(A::MessageActionChatAddUser(Box::new(types::MessageActionChatAddUser { users: vec![3] }))),
            service(A::MessageActionChatDeleteUser(types::MessageActionChatDeleteUser { user_id: 3 })),
            service(A::MessageActionChatEditTitle(Box::new(types::MessageActionChatEditTitle { title: "T".into() }))),
            service(A::MessageActionChatEditPhoto(Box::new(types::MessageActionChatEditPhoto {
                photo: photo.clone(),
            }))),
            service(A::MessageActionChatDeletePhoto),
            service(A::MessageActionChatCreate(Box::new(types::MessageActionChatCreate {
                title: "T".into(),
                users: Vec::new(),
            }))),
            service(channel_create()),
            types::MessageService { post: true, ..service(channel_create()) },
            service(A::MessageActionChatMigrateTo(types::MessageActionChatMigrateTo { channel_id: 3 })),
            service(A::MessageActionChannelMigrateFrom(Box::new(types::MessageActionChannelMigrateFrom {
                title: "T".into(),
                chat_id: 3,
            }))),
            types::MessageService { reply_to: reply_to(5), ..service(A::MessageActionPinMessage) },
            service(A::MessageActionSetMessagesTTL(Box::new(types::MessageActionSetMessagesTTL {
                period: 60,
                auto_setting_from: None,
            }))),
            service(A::MessageActionGroupCallScheduled(Box::new(types::MessageActionGroupCallScheduled {
                call: call(),
                schedule_date: 9,
            }))),
            service(A::MessageActionGroupCall(Box::new(types::MessageActionGroupCall {
                call: call(),
                duration: None,
            }))),
            service(A::MessageActionGroupCall(Box::new(types::MessageActionGroupCall {
                call: call(),
                duration: Some(60),
            }))),
            service(A::MessageActionInviteToGroupCall(Box::new(types::MessageActionInviteToGroupCall {
                call: call(),
                users: vec![3],
            }))),
            bot_allowed(None),
            bot_allowed(Some("example.com")),
            service(A::MessageActionRequestedPeerSentMe(Box::new(types::MessageActionRequestedPeerSentMe {
                button_id: 1,
                peers: vec![base::RequestedPeer::RequestedPeerUser(Box::new(user))],
            }))),
            service(A::MessageActionRequestedPeerSentMe(Box::new(types::MessageActionRequestedPeerSentMe {
                button_id: 1,
                peers: vec![base::RequestedPeer::RequestedPeerChannel(Box::new(channel))],
            }))),
            service(A::MessageActionGeoProximityReached(Box::new(types::MessageActionGeoProximityReached {
                from_id: get_peer(3),
                to_id: get_peer(4),
                distance: 10,
            }))),
            service(A::MessageActionTopicCreate(Box::new(types::MessageActionTopicCreate {
                title_missing: false,
                title: "T".into(),
                icon_color: 0,
                icon_emoji_id: None,
            }))),
            service(topic_edit(Some("T"), None, None)),
            service(topic_edit(None, Some(true), None)),
            service(topic_edit(None, Some(false), None)),
            service(topic_edit(None, None, Some(true))),
            service(topic_edit(None, None, Some(false))),
            service(A::MessageActionTodoCompletions(Box::new(types::MessageActionTodoCompletions {
                completed: vec![1],
                incompleted: Vec::new(),
            }))),
            service(A::MessageActionTodoAppendTasks(Box::new(types::MessageActionTodoAppendTasks {
                list: vec![item(4, "Visa")],
            }))),
            service(A::MessageActionChangeCommunity(Box::new(types::MessageActionChangeCommunity {
                community_id: Some(7),
            }))),
            service(A::MessageActionChangeCommunity(Box::new(types::MessageActionChangeCommunity {
                community_id: None,
            }))),
            service(A::MessageActionChatJoinedViaCommunity(types::MessageActionChatJoinedViaCommunity {
                community_id: 7,
            })),
            service(A::MessageActionStarGift(Box::new(star_gift()))),
            service(A::MessageActionStarGiftUnique(Box::new(unique_gift()))),
            service(A::MessageActionManagedBotCreated(types::MessageActionManagedBotCreated { bot_id: 4 })),
            service(A::MessageActionPaymentSentMe(Box::new(types::MessageActionPaymentSentMe {
                recurring_init: false,
                recurring_used: false,
                currency: "XTR".into(),
                total_amount: 5,
                payload: b"order".to_vec(),
                info: None,
                shipping_option_id: None,
                charge: charge(),
                subscription_until_date: None,
            }))),
            service(A::MessageActionPaymentRefunded(Box::new(types::MessageActionPaymentRefunded {
                peer: get_peer(9),
                currency: "XTR".into(),
                total_amount: 5,
                payload: None,
                charge: charge(),
            }))),
            service(approval(false, false)),
            service(approval(false, true)),
            service(approval(true, false)),
            service(A::MessageActionSuggestedPostSuccess(Box::new(types::MessageActionSuggestedPostSuccess {
                price: base::StarsAmount::StarsTonAmount(types::StarsTonAmount { amount: 5 }),
            }))),
            service(A::MessageActionSuggestedPostRefund(types::MessageActionSuggestedPostRefund {
                payer_initiated: true,
            })),
            service(A::MessageActionBoostApply(types::MessageActionBoostApply { boosts: 2 })),
            service(A::MessageActionGameScore(Box::new(types::MessageActionGameScore { game_id: 7, score: 42 }))),
            service(A::MessageActionWebViewDataSentMe(Box::new(types::MessageActionWebViewDataSentMe {
                text: "Go".into(),
                data: "{}".into(),
            }))),
            service(A::MessageActionNewCreatorPending(types::MessageActionNewCreatorPending { new_creator_id: 4 })),
            service(A::MessageActionChangeCreator(types::MessageActionChangeCreator { new_creator_id: 4 })),
            service(A::MessageActionContactSignUp),
            service(A::MessageActionCustomAction(Box::new(types::MessageActionCustomAction { message: "hi".into() }))),
            service(A::MessageActionGiftCode(Box::new(gift_code()))),
            service(A::MessageActionGiftPremium(Box::new(premium()))),
            service(A::MessageActionGiftStars(Box::new(types::MessageActionGiftStars {
                currency: "USD".into(),
                amount: 5,
                stars: 100,
                crypto_currency: None,
                crypto_amount: None,
                transaction_id: None,
            }))),
            service(A::MessageActionGiftTon(Box::new(types::MessageActionGiftTon {
                currency: "USD".into(),
                amount: 5,
                crypto_currency: "TON".into(),
                crypto_amount: 1_000_000_000,
                transaction_id: Some("tx".into()),
            }))),
            service(A::MessageActionGiveawayLaunch(Box::new(types::MessageActionGiveawayLaunch { stars: Some(500) }))),
            service(A::MessageActionGiveawayResults(Box::new(types::MessageActionGiveawayResults {
                stars: false,
                winners_count: 3,
                unclaimed_count: 0,
            }))),
            service(A::MessageActionPrizeStars(Box::new(types::MessageActionPrizeStars {
                unclaimed: false,
                stars: 50,
                transaction_id: "tx".into(),
                boost_peer: get_peer(-1000000000005),
                giveaway_msg_id: 7,
            }))),
            service(A::MessageActionHistoryClear),
            service(phone_call(None, None)),
            service(phone_call(Some(base::PhoneCallDiscardReason::PhoneCallDiscardReasonHangup), Some(60))),
            service(A::MessageActionConferenceCall(Box::new(types::MessageActionConferenceCall {
                missed: false,
                active: true,
                video: false,
                call_id: 1,
                duration: None,
                other_participants: None,
            }))),
            service(A::MessageActionScreenshotTaken),
            service(offer(unique_gift().gift, false, false)),
            service(A::MessageActionStarGiftPurchaseOfferDeclined(Box::new(
                types::MessageActionStarGiftPurchaseOfferDeclined {
                    expired: true,
                    gift: unique_gift().gift,
                    price: stars(5),
                },
            ))),
            service(A::MessageActionNoForwardsToggle(types::MessageActionNoForwardsToggle {
                prev_value: false,
                new_value: true,
            })),
            service(A::MessageActionNoForwardsRequest(types::MessageActionNoForwardsRequest {
                expired: false,
                prev_value: true,
                new_value: false,
            })),
            service(theme(base::ChatTheme::ChatTheme(Box::new(types::ChatTheme { emoticon: "🐱".into() })))),
            service(wallpaper(false, true)),
            service(A::MessageActionSuggestProfilePhoto(Box::new(types::MessageActionSuggestProfilePhoto { photo }))),
            service(A::MessageActionSuggestBirthday(Box::new(types::MessageActionSuggestBirthday {
                birthday: base::Birthday::Birthday(Box::new(types::Birthday { day: 31, month: 10, year: None })),
            }))),
            service(A::MessageActionPaidMessagesRefunded(Box::new(types::MessageActionPaidMessagesRefunded {
                count: 2,
                stars: 10,
            }))),
            service(paid_price(false)),
            types::MessageService { post: true, ..service(paid_price(true)) },
            service(A::MessageActionPollAppendAnswer(Box::new(types::MessageActionPollAppendAnswer {
                answer: poll_answer(),
            }))),
            service(A::MessageActionPollDeleteAnswer(Box::new(types::MessageActionPollDeleteAnswer {
                answer: poll_answer(),
            }))),
            service(A::MessageActionSecureValuesSent(Box::new(types::MessageActionSecureValuesSent {
                types: vec![base::SecureValueType::SecureValueTypePassport],
            }))),
            service(A::MessageActionGramTransfer(Box::new(types::MessageActionGramTransfer {
                comment_encrypted: false,
                amount: 5,
                peer_address: "EQa".into(),
                transaction_id: "tx".into(),
                comment: None,
            }))),
            service(A::MessageActionWalletTonConnectRequest(Box::new(types::MessageActionWalletTonConnectRequest {
                accepted: false,
                declined: true,
                session_id: 1,
                expires: 2,
                topic: None,
                trace_id: None,
                dapp_name: None,
            }))),
            service(A::MessageActionNoPaidMessagesTokenUsed),
            service(A::MessageActionSecureValuesSentMe(Box::new(types::MessageActionSecureValuesSentMe {
                values: Vec::new(),
                credentials: base::SecureCredentialsEncrypted::SecureCredentialsEncrypted(Box::new(
                    types::SecureCredentialsEncrypted { data: Vec::new(), hash: Vec::new(), secret: Vec::new() },
                )),
            }))),
        ]
    }

    #[test]
    fn every_service_kind_parses_to_its_own_variant() {
        let entities = Entities::default();
        let mut seen = HashSet::new();
        for raw in every_kind() {
            let kind = ServiceKind::of(&raw.action, Some(Chat::kind_of(&raw.peer_id, &entities, raw.post)));
            let action = ServiceAction::parse(&raw, &entities).expect("a well-formed payload");
            assert_eq!(action.kind(), kind);
            seen.insert(kind);
        }
        // Discriminants run from 1 up to `Unknown`, so a new kind fails here until it gets a sample above.
        assert_eq!(seen.len(), ServiceKind::Unknown as usize - 1);
    }

    #[test]
    fn malformed_payloads_keep_their_kind_without_an_action() {
        use base::MessageAction as A;
        let entities = Entities::default();
        let kind_and_action =
            |raw: types::MessageService| (ServiceKind::of(&raw.action, None), ServiceAction::parse(&raw, &entities));
        let empty = types::MessageActionChatEditPhoto { photo: base::Photo::PhotoEmpty(types::PhotoEmpty { id: 1 }) };
        let joined = types::MessageService { from_id: None, ..service(A::MessageActionChatJoinedByRequest) };
        let cases = [
            (service(A::MessageActionPinMessage), ServiceKind::PinnedMessage),
            (service(A::MessageActionChatEditPhoto(Box::new(empty))), ServiceKind::NewChatPhoto),
            (joined, ServiceKind::NewChatMembers),
            (service(offer(star_gift().gift, false, false)), ServiceKind::UpgradedGiftPurchaseOffer),
            (
                service(A::MessageActionSuggestProfilePhoto(Box::new(types::MessageActionSuggestProfilePhoto {
                    photo: base::Photo::PhotoEmpty(types::PhotoEmpty { id: 1 }),
                }))),
                ServiceKind::SuggestProfilePhoto,
            ),
        ];
        for (raw, kind) in cases {
            assert_eq!(kind_and_action(raw), (kind, None));
        }
    }

    #[test]
    fn unmapped_actions_are_unknown_with_their_constructor_id() {
        use crate::raw::Deserializable;
        let receipt = types::MessageActionPaymentSent {
            recurring_init: false,
            recurring_used: false,
            currency: "XTR".into(),
            total_amount: 5,
            invoice_slug: None,
            subscription_until_date: None,
        };
        let empty = base::MessageAction::deserialize(&mut &0xb6ae_f7b0_u32.to_le_bytes()[..]).unwrap();
        let cases =
            [(empty, 0xb6ae_f7b0), (base::MessageAction::MessageActionPaymentSent(Box::new(receipt)), 0xc624_b16e)];
        for (action, id) in cases {
            let action = parse(action).expect("an unmapped action still makes a service message");
            assert!(matches!(action, ServiceAction::Unknown { constructor_id } if constructor_id == id));
            assert_eq!(action.kind(), ServiceKind::Unknown);
        }
    }

    #[test]
    fn maps_chat_actions() {
        use base::MessageAction as A;
        let link = A::MessageActionChatJoinedByLink(types::MessageActionChatJoinedByLink { inviter_id: 3 });
        assert!(matches!(parse(link), Some(ServiceAction::NewChatMembers(u)) if u.iter().map(|u| u.id).eq([9])));
        let pin = types::MessageService { reply_to: reply_to(5), ..service(A::MessageActionPinMessage) };
        assert_eq!(ServiceAction::parse(&pin, &Entities::default()), Some(ServiceAction::PinnedMessage(5)));

        let channel = types::RequestedPeerChannel { channel_id: 3, title: None, username: None, photo: None };
        let peers = vec![base::RequestedPeer::RequestedPeerChannel(Box::new(channel))];
        let shared = types::MessageActionRequestedPeerSentMe { button_id: 4, peers };
        let Some(ServiceAction::ChatShared(chat)) = parse(A::MessageActionRequestedPeerSentMe(Box::new(shared))) else {
            panic!("expected a shared chat")
        };
        assert_eq!((chat.request_id, chat.chat_id, chat.title), (4, -1000000000003, None));
        let ids = types::MessageActionRequestedPeer { button_id: 4, peers: vec![get_peer(7), get_peer(8)] };
        let ids = parse(A::MessageActionRequestedPeer(Box::new(ids)));
        assert!(matches!(ids, Some(ServiceAction::UsersShared(u)) if u.users.iter().map(|u| u.user_id).eq([7, 8])));
        let mixed = types::MessageActionRequestedPeer { button_id: 4, peers: vec![get_peer(7), get_peer(-8)] };
        let mixed = parse(A::MessageActionRequestedPeer(Box::new(mixed)));
        assert!(matches!(mixed, Some(ServiceAction::ChatShared(c)) if c.chat_id == -8));

        let site = types::MessageActionBotAllowed {
            attach_menu: false,
            from_request: false,
            domain: Some("example.com".into()),
            app: None,
        };
        let site = parse(A::MessageActionBotAllowed(Box::new(site)));
        assert_eq!(site, Some(ServiceAction::ConnectedWebsite("example.com".into())));
        assert_eq!(parse(A::MessageActionScreenshotTaken), Some(ServiceAction::ScreenshotTaken));
    }

    #[test]
    fn maps_suggested_post_actions() {
        let price = Some(SuggestedPostPrice::Ton(5));
        let approved = SuggestedPostApproved { price, send_date: Some(9) };
        assert!(
            matches!(parse(approval(false, false)), Some(ServiceAction::SuggestedPostApproved(a)) if a == approved)
        );
        assert!(matches!(
            parse(approval(false, true)),
            Some(ServiceAction::SuggestedPostApprovalFailed(f)) if f.price == price
        ));
        assert!(matches!(
            parse(approval(true, false)),
            Some(ServiceAction::SuggestedPostDeclined(d)) if d.comment.as_deref() == Some("no")
        ));

        let refund = |payer_initiated| {
            let refund = types::MessageActionSuggestedPostRefund { payer_initiated };
            match parse(base::MessageAction::MessageActionSuggestedPostRefund(refund)) {
                Some(ServiceAction::SuggestedPostRefunded(r)) => Some(r.reason),
                _ => None,
            }
        };
        assert_eq!(refund(true), Some(SuggestedPostRefundReason::PaymentRefunded));
        assert_eq!(refund(false), Some(SuggestedPostRefundReason::PostDeleted));
    }

    #[test]
    fn maps_checklist_actions() {
        let done = types::MessageActionTodoCompletions { completed: vec![1, 3], incompleted: vec![2] };
        let expected = ChecklistTasksDone { marked_as_done_task_ids: vec![1, 3], marked_as_not_done_task_ids: vec![2] };
        let done = parse(base::MessageAction::MessageActionTodoCompletions(Box::new(done)));
        assert_eq!(done, Some(ServiceAction::ChecklistTasksDone(expected)));

        let added = types::MessageActionTodoAppendTasks { list: vec![item(4, "Visa")] };
        let added = parse(base::MessageAction::MessageActionTodoAppendTasks(Box::new(added)));
        let tasks = vec![ChecklistTask::parse(&item(4, "Visa"), &Entities::default())];
        assert_eq!(added, Some(ServiceAction::ChecklistTasksAdded(ChecklistTasksAdded { tasks })));
    }

    #[test]
    fn paid_message_price_follows_the_chat_kind() {
        let entities = Entities::default();
        let channel = types::MessageService { post: true, ..service(paid_price(true)) };
        assert_eq!(
            ServiceAction::parse(&channel, &entities),
            Some(ServiceAction::DirectMessagePriceChanged { is_enabled: true, paid_message_star_count: 10 })
        );
        assert_eq!(
            parse(paid_price(false)),
            Some(ServiceAction::PaidMessagePriceChanged { paid_message_star_count: 10 })
        );
    }

    #[test]
    fn maps_phone_calls() {
        use crate::enums::PhoneCallDiscardReason;
        assert_eq!(parse(phone_call(None, None)), Some(ServiceAction::PhoneCallStarted { id: 8, is_video: true }));
        let ended = |duration| match parse(phone_call(
            Some(base::PhoneCallDiscardReason::PhoneCallDiscardReasonMissed),
            duration,
        )) {
            Some(ServiceAction::PhoneCallEnded(e)) => Some((e.id, e.reason, e.duration)),
            _ => None,
        };
        assert_eq!(ended(None), Some((8, PhoneCallDiscardReason::Missed, None)));
        assert_eq!(ended(Some(60)), Some((8, PhoneCallDiscardReason::Missed, Some(60))));
    }

    #[test]
    fn maps_gift_actions() {
        use crate::enums::GiftPurchaseOfferState;
        let state = |accepted, declined| match parse(offer(unique_gift().gift, accepted, declined)) {
            Some(ServiceAction::UpgradedGiftPurchaseOffer(o)) => Some((o.state, o.price.amount, o.gift.gift_id)),
            _ => None,
        };
        assert_eq!(state(false, false), Some((GiftPurchaseOfferState::Pending, 5, 2)));
        assert_eq!(state(true, false), Some((GiftPurchaseOfferState::Accepted, 5, 2)));
        assert_eq!(state(false, true), Some((GiftPurchaseOfferState::Rejected, 5, 2)));

        let Some(ServiceAction::GiftedPremium(gifted)) =
            parse(base::MessageAction::MessageActionGiftPremium(Box::new(premium())))
        else {
            panic!("expected gifted Premium")
        };
        assert_eq!((gifted.day_count, gifted.text.as_deref()), (30, Some("enjoy")));
        let Some(ServiceAction::PremiumGiftCode(code)) =
            parse(base::MessageAction::MessageActionGiftCode(Box::new(gift_code())))
        else {
            panic!("expected a gift code")
        };
        assert_eq!((code.code.as_str(), code.is_from_giveaway), ("code", true));
        assert_eq!(code.creator.map(|c| c.id), Some(-1000000000005));
    }

    #[test]
    fn maps_themes_backgrounds_and_poll_options() {
        let named = theme(base::ChatTheme::ChatTheme(Box::new(types::ChatTheme { emoticon: "🐱".into() })));
        let Some(ServiceAction::ChatSetTheme(named)) = parse(named) else { panic!("expected a theme") };
        assert_eq!((named.name.as_deref(), named.gift.is_none()), (Some("🐱"), true));
        let gift = theme(base::ChatTheme::ChatThemeUniqueGift(Box::new(types::ChatThemeUniqueGift {
            gift: unique_gift().gift,
            theme_settings: Vec::new(),
        })));
        let Some(ServiceAction::ChatSetTheme(gift)) = parse(gift) else { panic!("expected a theme") };
        assert_eq!((gift.name, gift.gift.map(|g| g.gift_id)), (None, Some(2)));

        let Some(ServiceAction::ChatBackgroundSet { background, is_same, is_for_both }) = parse(wallpaper(true, false))
        else {
            panic!("expected a background")
        };
        assert_eq!((background.id, background.is_dark, is_same, is_for_both), (1, true, true, false));

        let added = types::MessageActionPollAppendAnswer { answer: poll_answer() };
        let Some(ServiceAction::PollOptionAdded(option)) =
            parse(base::MessageAction::MessageActionPollAppendAnswer(Box::new(added)))
        else {
            panic!("expected a poll option")
        };
        assert_eq!((option.text.as_str(), option.data.as_slice(), option.voter_count), ("Tea", &[2u8][..], 0));
    }

    #[test]
    fn parses_payload_structs() {
        use crate::types::{Chat, GiftedGrams, GiftedStars, GiveawayPrizeStars, WebAppData};
        let entities = Entities::default();
        let actions: Vec<_> = every_kind().iter().filter_map(|raw| ServiceAction::parse(raw, &entities)).collect();
        let stars = GiftedStars {
            currency: "USD".into(),
            amount: 5,
            cryptocurrency: None,
            cryptocurrency_amount: None,
            star_count: 100,
            transaction_id: None,
        };
        let grams = GiftedGrams { ton_amount: 1_000_000_000, transaction_id: Some("tx".into()) };
        let prize = GiveawayPrizeStars {
            star_count: 50,
            transaction_id: "tx".into(),
            boosted_chat: Chat::parse(&get_peer(-1000000000005), &entities, true),
            giveaway_message_id: 7,
            is_unclaimed: false,
        };
        let web = WebAppData { button_text: "Go".into(), data: Some("{}".into()) };
        let expected = [
            ServiceAction::GiftedStars(Box::new(stars)),
            ServiceAction::GiftedGrams(Box::new(grams)),
            ServiceAction::GiveawayPrizeStars(Box::new(prize)),
            ServiceAction::ConferenceCall { id: 1, is_missed: false, is_active: true, is_video: false, duration: None },
            ServiceAction::ChatHasProtectedContentToggled {
                old_has_protected_content: false,
                new_has_protected_content: true,
            },
            ServiceAction::WebAppData(Box::new(web)),
        ];
        for action in expected {
            assert!(actions.contains(&action), "missing {action:?}");
        }
    }
}
