use crate::dispatcher::Entities;
use crate::enums::SuggestedPostRefundReason;
use crate::raw::base;
use crate::types::{
    Community, ForumTopicCreated, ForumTopicEdited, SuggestedPostApprovalFailed, SuggestedPostApproved,
    SuggestedPostDeclined, SuggestedPostPaid, SuggestedPostPrice, SuggestedPostRefunded, User,
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
}

impl ServiceAction {
    pub(crate) fn parse(action: &base::MessageAction, entities: &Entities) -> Option<ServiceAction> {
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
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ServiceAction;
    use crate::dispatcher::Entities;
    use crate::enums::SuggestedPostRefundReason;
    use crate::raw::{base, types};
    use crate::types::{SuggestedPostApproved, SuggestedPostPrice};

    #[test]
    fn maps_suggested_post_actions() {
        let parse = |action| ServiceAction::parse(&action, &Entities::default()).unwrap();
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
}
