use crate::dispatcher::Entities;
use crate::enums::SuggestedPostRefundReason;
use crate::raw::base;
use crate::types::{
    ChecklistTask, ChecklistTasksAdded, ChecklistTasksDone, Community, ForumTopicCreated, ForumTopicEdited, GiftInfo,
    RefundedPayment, SuccessfulPayment, SuggestedPostApprovalFailed, SuggestedPostApproved, SuggestedPostDeclined,
    SuggestedPostPaid, SuggestedPostPrice, SuggestedPostRefunded, UniqueGiftInfo, User,
};

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceAction {
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
    GameHighScore { game_id: i64, score: i32 },
}

impl ServiceAction {
    /// `msg_id` is the service message's own id, which addresses a gift a user keeps.
    pub(crate) fn parse(action: &base::MessageAction, msg_id: i32, entities: &Entities) -> Option<ServiceAction> {
        use base::MessageAction as A;
        Some(match action {
            A::MessageActionChangeCommunity(a) => match a.community_id {
                Some(id) => ServiceAction::CommunityChatAdded(Community::parse(id, entities)),
                // An absent community_id means the chat left its community.
                None => ServiceAction::CommunityChatRemoved,
            },
            A::MessageActionChatJoinedViaCommunity(a) => {
                ServiceAction::CommunityChatJoined(Community::parse(a.community_id, entities))
            }
            A::MessageActionManagedBotCreated(a) => ServiceAction::ManagedBotCreated(User::parse(a.bot_id, entities)),
            A::MessageActionSuggestedPostApproval(a) => {
                let price = a.price.as_ref().map(SuggestedPostPrice::parse);
                if a.rejected {
                    ServiceAction::SuggestedPostDeclined(SuggestedPostDeclined { comment: a.reject_comment.clone() })
                } else if a.balance_too_low {
                    ServiceAction::SuggestedPostApprovalFailed(SuggestedPostApprovalFailed { price })
                } else {
                    ServiceAction::SuggestedPostApproved(SuggestedPostApproved { price, send_date: a.schedule_date })
                }
            }
            A::MessageActionSuggestedPostSuccess(a) => {
                ServiceAction::SuggestedPostPaid(SuggestedPostPaid { price: SuggestedPostPrice::parse(&a.price) })
            }
            A::MessageActionSuggestedPostRefund(a) => {
                let reason = if a.payer_initiated {
                    SuggestedPostRefundReason::PaymentRefunded
                } else {
                    SuggestedPostRefundReason::PostDeleted
                };
                ServiceAction::SuggestedPostRefunded(SuggestedPostRefunded { reason })
            }
            A::MessageActionTopicCreate(a) => ServiceAction::ForumTopicCreated(ForumTopicCreated {
                title: a.title.clone(),
                icon_color: a.icon_color,
                icon_emoji_id: a.icon_emoji_id,
                is_name_implicit: a.title_missing,
            }),
            // One edit can set several flags; ServiceKind reads them in this order, so filters and `service` agree.
            A::MessageActionTopicEdit(a) => match (a.closed, a.hidden) {
                (Some(true), _) => ServiceAction::ForumTopicClosed,
                (Some(false), _) => ServiceAction::ForumTopicReopened,
                (_, Some(true)) => ServiceAction::GeneralForumTopicHidden,
                (_, Some(false)) => ServiceAction::GeneralForumTopicUnhidden,
                _ => ServiceAction::ForumTopicEdited(ForumTopicEdited {
                    title: a.title.clone(),
                    icon_emoji_id: a.icon_emoji_id,
                }),
            },
            A::MessageActionStarGift(a) => ServiceAction::Gift(Box::new(GiftInfo::parse(a, msg_id, entities)?)),
            A::MessageActionStarGiftUnique(a) => {
                ServiceAction::UniqueGift(Box::new(UniqueGiftInfo::parse(a, msg_id, entities)?))
            }
            A::MessageActionPaymentSentMe(a) => ServiceAction::SuccessfulPayment(Box::new(SuccessfulPayment::parse(a))),
            A::MessageActionPaymentRefunded(a) => ServiceAction::RefundedPayment(Box::new(RefundedPayment::parse(a))),
            A::MessageActionTodoCompletions(a) => ServiceAction::ChecklistTasksDone(ChecklistTasksDone {
                marked_as_done_task_ids: a.completed.clone(),
                marked_as_not_done_task_ids: a.incompleted.clone(),
            }),
            A::MessageActionTodoAppendTasks(a) => ServiceAction::ChecklistTasksAdded(ChecklistTasksAdded {
                tasks: a.list.iter().map(|item| ChecklistTask::parse(item, entities)).collect(),
            }),
            A::MessageActionGameScore(a) => ServiceAction::GameHighScore { game_id: a.game_id, score: a.score },
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::checklist::tests::item;
    use super::ServiceAction;
    use crate::dispatcher::Entities;
    use crate::enums::{ServiceKind, SuggestedPostRefundReason};
    use crate::raw::{base, types};
    use crate::types::{
        ChecklistTask, ChecklistTasksAdded, ChecklistTasksDone, SuggestedPostApproved, SuggestedPostPrice,
    };

    #[test]
    fn maps_suggested_post_actions() {
        let parse = |action| ServiceAction::parse(&action, 1, &Entities::default()).unwrap();
        let approval = |rejected, balance_too_low| {
            parse(base::MessageAction::MessageActionSuggestedPostApproval(Box::new(
                types::MessageActionSuggestedPostApproval {
                    rejected,
                    balance_too_low,
                    reject_comment: rejected.then(|| "no".into()),
                    schedule_date: Some(9),
                    price: Some(base::StarsAmount::StarsTonAmount(Box::new(types::StarsTonAmount { amount: 5 }))),
                },
            )))
        };
        let price = Some(SuggestedPostPrice::Ton(5));
        let approved = SuggestedPostApproved { price, send_date: Some(9) };
        assert!(matches!(approval(false, false), ServiceAction::SuggestedPostApproved(a) if a == approved));
        assert!(matches!(approval(false, true), ServiceAction::SuggestedPostApprovalFailed(f) if f.price == price));
        assert!(matches!(
            approval(true, false),
            ServiceAction::SuggestedPostDeclined(d) if d.comment.as_deref() == Some("no")
        ));

        let refund = |payer_initiated| {
            let refund = types::MessageActionSuggestedPostRefund { payer_initiated };
            match parse(base::MessageAction::MessageActionSuggestedPostRefund(Box::new(refund))) {
                ServiceAction::SuggestedPostRefunded(r) => Some(r.reason),
                _ => None,
            }
        };
        assert_eq!(refund(true), Some(SuggestedPostRefundReason::PaymentRefunded));
        assert_eq!(refund(false), Some(SuggestedPostRefundReason::PostDeleted));
    }

    #[test]
    fn maps_checklist_actions() {
        let entities = Entities::default();
        let parse = |action| ServiceAction::parse(&action, 1, &entities).unwrap();
        let done = types::MessageActionTodoCompletions { completed: vec![1, 3], incompleted: vec![2] };
        let expected = ChecklistTasksDone { marked_as_done_task_ids: vec![1, 3], marked_as_not_done_task_ids: vec![2] };
        let done = parse(base::MessageAction::MessageActionTodoCompletions(Box::new(done)));
        assert_eq!(done, ServiceAction::ChecklistTasksDone(expected));

        let added = types::MessageActionTodoAppendTasks { list: vec![item(4, "Visa")] };
        let added = parse(base::MessageAction::MessageActionTodoAppendTasks(Box::new(added)));
        let tasks = vec![ChecklistTask::parse(&item(4, "Visa"), &entities)];
        assert_eq!(added, ServiceAction::ChecklistTasksAdded(ChecklistTasksAdded { tasks }));
    }

    #[test]
    fn game_scores_agree_with_their_service_kind() {
        let action = base::MessageAction::MessageActionGameScore(Box::new(types::MessageActionGameScore {
            game_id: 7,
            score: 42,
        }));
        let parsed = ServiceAction::parse(&action, 1, &Entities::default());
        assert_eq!(parsed, Some(ServiceAction::GameHighScore { game_id: 7, score: 42 }));
        assert_eq!(ServiceKind::of(&action, None), ServiceKind::GameHighScore);
    }
}
