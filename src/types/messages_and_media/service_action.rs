use crate::dispatcher::Entities;
use crate::enums::{ServiceKind, SuggestedPostRefundReason};
use crate::raw::{base, types};
use crate::types::{
    Chat, ChatShared, ChecklistTask, ChecklistTasksAdded, ChecklistTasksDone, Community, ForumTopicCreated,
    ForumTopicEdited, GiftInfo, Photo, ProximityAlertTriggered, RefundedPayment, SharedUser, SuccessfulPayment,
    SuggestedPostApprovalFailed, SuggestedPostApproved, SuggestedPostDeclined, SuggestedPostPaid, SuggestedPostPrice,
    SuggestedPostRefunded, UniqueGiftInfo, User, UsersShared, WriteAccessAllowed,
};
use crate::utils::{get_channel_id, get_peer_id};

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum ServiceAction {
    NewChatMembers(Vec<User>),
    LeftChatMember(User),
    NewChatTitle(String),
    NewChatPhoto(Box<Photo>),
    DeleteChatPhoto,
    GroupChatCreated,
    SupergroupChatCreated,
    ChannelChatCreated,
    MigrateToChatId(i64),
    MigrateFromChatId(i64),
    PinnedMessage(i32),
    /// In seconds; 0 turns auto-deletion off.
    AutoDeleteTimerChanged(i32),
    VideoChatScheduled {
        start_date: i32,
    },
    VideoChatStarted,
    VideoChatEnded {
        /// In seconds.
        duration: i32,
    },
    VideoChatParticipantsInvited(Vec<User>),
    WriteAccessAllowed(WriteAccessAllowed),
    /// The domain the user logged in to with Telegram Login.
    ConnectedWebsite(String),
    UsersShared(UsersShared),
    ChatShared(Box<ChatShared>),
    ProximityAlertTriggered(Box<ProximityAlertTriggered>),
    CommunityChatAdded(Community),
    CommunityChatRemoved,
    CommunityChatJoined(Community),
    ManagedBotCreated(User),
    SuggestedPostApproved(SuggestedPostApproved),
    SuggestedPostApprovalFailed(SuggestedPostApprovalFailed),
    SuggestedPostDeclined(SuggestedPostDeclined),
    SuggestedPostPaid(SuggestedPostPaid),
    SuggestedPostRefunded(SuggestedPostRefunded),
    ForumTopicCreated(ForumTopicCreated),
    ForumTopicEdited(ForumTopicEdited),
    ForumTopicClosed,
    ForumTopicReopened,
    GeneralForumTopicHidden,
    GeneralForumTopicUnhidden,
    Gift(Box<GiftInfo>),
    UniqueGift(Box<UniqueGiftInfo>),
    SuccessfulPayment(Box<SuccessfulPayment>),
    RefundedPayment(Box<RefundedPayment>),
    ChecklistTasksDone(ChecklistTasksDone),
    ChecklistTasksAdded(ChecklistTasksAdded),
    GameHighScore {
        game_id: i64,
        score: i32,
    },
    BoostAdded(i32),
}

impl ServiceAction {
    /// The variant `ServiceKind::of` names, so `filters::service_kind` and `service` always agree. `None` for
    /// `ServiceKind::Unknown`, and for a known kind only when its payload is malformed (a pin without its header).
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
            _ => return None,
        })
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
            price: Some(base::StarsAmount::StarsTonAmount(Box::new(types::StarsTonAmount { amount: 5 }))),
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
            service(A::MessageActionChatDeleteUser(Box::new(types::MessageActionChatDeleteUser { user_id: 3 }))),
            service(A::MessageActionChatEditTitle(Box::new(types::MessageActionChatEditTitle { title: "T".into() }))),
            service(A::MessageActionChatEditPhoto(Box::new(types::MessageActionChatEditPhoto { photo }))),
            service(A::MessageActionChatDeletePhoto),
            service(A::MessageActionChatCreate(Box::new(types::MessageActionChatCreate {
                title: "T".into(),
                users: Vec::new(),
            }))),
            service(channel_create()),
            types::MessageService { post: true, ..service(channel_create()) },
            service(A::MessageActionChatMigrateTo(Box::new(types::MessageActionChatMigrateTo { channel_id: 3 }))),
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
            service(A::MessageActionChatJoinedViaCommunity(Box::new(types::MessageActionChatJoinedViaCommunity {
                community_id: 7,
            }))),
            service(A::MessageActionStarGift(Box::new(star_gift()))),
            service(A::MessageActionStarGiftUnique(Box::new(unique_gift()))),
            service(A::MessageActionManagedBotCreated(Box::new(types::MessageActionManagedBotCreated { bot_id: 4 }))),
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
                price: base::StarsAmount::StarsTonAmount(Box::new(types::StarsTonAmount { amount: 5 })),
            }))),
            service(A::MessageActionSuggestedPostRefund(Box::new(types::MessageActionSuggestedPostRefund {
                payer_initiated: true,
            }))),
            service(A::MessageActionBoostApply(Box::new(types::MessageActionBoostApply { boosts: 2 }))),
            service(A::MessageActionGameScore(Box::new(types::MessageActionGameScore { game_id: 7, score: 42 }))),
        ]
    }

    #[test]
    fn every_service_kind_parses_to_its_own_variant() {
        let entities = Entities::default();
        let mut seen = HashSet::new();
        for raw in every_kind() {
            let kind = ServiceKind::of(&raw.action, Some(Chat::kind_of(&raw.peer_id, &entities, raw.post)));
            let action = format!("{:?}", ServiceAction::parse(&raw, &entities).expect("a well-formed payload"));
            assert_eq!(action.split(['(', ' ']).next(), Some(format!("{kind:?}").as_str()));
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
        let empty =
            types::MessageActionChatEditPhoto { photo: base::Photo::PhotoEmpty(Box::new(types::PhotoEmpty { id: 1 })) };
        let joined = types::MessageService { from_id: None, ..service(A::MessageActionChatJoinedByRequest) };
        let premium = types::MessageActionGiftPremium {
            currency: "USD".into(),
            amount: 5,
            days: 30,
            crypto_currency: None,
            crypto_amount: None,
            message: None,
        };
        let receipt = types::MessageActionPaymentSent {
            recurring_init: false,
            recurring_used: false,
            currency: "XTR".into(),
            total_amount: 5,
            invoice_slug: None,
            subscription_until_date: None,
        };
        let cases = [
            (service(A::MessageActionPinMessage), ServiceKind::PinnedMessage),
            (service(A::MessageActionChatEditPhoto(Box::new(empty))), ServiceKind::NewChatPhoto),
            (joined, ServiceKind::NewChatMembers),
            (service(A::MessageActionGiftPremium(Box::new(premium))), ServiceKind::Unknown),
            (service(A::MessageActionPaymentSent(Box::new(receipt))), ServiceKind::Unknown),
        ];
        for (raw, kind) in cases {
            assert_eq!(kind_and_action(raw), (kind, None));
        }
    }

    #[test]
    fn maps_chat_actions() {
        use base::MessageAction as A;
        let link = A::MessageActionChatJoinedByLink(Box::new(types::MessageActionChatJoinedByLink { inviter_id: 3 }));
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
        assert_eq!(parse(A::MessageActionScreenshotTaken), None);
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
            match parse(base::MessageAction::MessageActionSuggestedPostRefund(Box::new(refund))) {
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
}
