use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::base;
use crate::types::User;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatInviteLink {
    pub invite_link: String,
    pub creator: User,
    pub date: i32,
    pub creates_join_request: bool,
    pub is_primary: bool,
    pub is_revoked: bool,
    pub name: Option<String>,
    pub expire_date: Option<i32>,
    pub member_limit: Option<i32>,
    pub member_count: Option<i32>,
    pub pending_join_request_count: Option<i32>,
    /// Seconds a subscription lasts before it renews.
    pub subscription_period: Option<i32>,
    pub subscription_price: Option<i64>,
}

impl ChatInviteLink {
    pub(crate) fn parse(invite: base::ExportedChatInvite, entities: &Entities) -> Result<ChatInviteLink> {
        let base::ExportedChatInvite::ChatInviteExported(i) = invite else {
            return Err(Error::Unexpected("public join requests in place of an invite link"));
        };
        let i = *i;
        let pricing = i.subscription_pricing.map(|base::StarsSubscriptionPricing::StarsSubscriptionPricing(p)| *p);
        Ok(ChatInviteLink {
            invite_link: i.link,
            creator: User::parse(i.admin_id, entities),
            date: i.date,
            creates_join_request: i.request_needed,
            is_primary: i.permanent,
            is_revoked: i.revoked,
            name: i.title,
            expire_date: i.expire_date,
            member_limit: i.usage_limit,
            member_count: i.usage,
            pending_join_request_count: i.requested,
            subscription_period: pricing.as_ref().map(|p| p.period),
            subscription_price: pricing.map(|p| p.amount),
        })
    }

    pub(crate) fn parse_edited(client: &Client, raw: base::messages::ExportedChatInvite) -> Result<ChatInviteLink> {
        // Revoking the primary link also returns its replacement; the caller asked about the revoked one.
        let (invite, users) = match raw {
            base::messages::ExportedChatInvite::ExportedChatInvite(r) => (r.invite, r.users),
            base::messages::ExportedChatInvite::ExportedChatInviteReplaced(r) => (r.invite, r.users),
        };
        ChatInviteLink::parse(invite, &Entities::new(client, users, Vec::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::ChatInviteLink;
    use crate::Error;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};

    #[test]
    fn parses_subscription_link() {
        let pricing = types::StarsSubscriptionPricing { period: 2_592_000, amount: 50 };
        let invite = types::ChatInviteExported {
            revoked: false,
            permanent: false,
            request_needed: false,
            link: "https://t.me/+abc".into(),
            admin_id: 7,
            date: 1_700_000_000,
            start_date: None,
            expire_date: Some(1_800_000_000),
            usage_limit: None,
            usage: Some(3),
            requested: None,
            subscription_expired: None,
            title: Some("paid".into()),
            subscription_pricing: Some(base::StarsSubscriptionPricing::StarsSubscriptionPricing(Box::new(pricing))),
        };
        let raw = base::ExportedChatInvite::ChatInviteExported(Box::new(invite));
        let link = ChatInviteLink::parse(raw, &Entities::default()).unwrap();
        assert_eq!((link.invite_link.as_str(), link.name.as_deref()), ("https://t.me/+abc", Some("paid")));
        assert_eq!((link.creator.id, link.expire_date, link.member_count), (7, Some(1_800_000_000), Some(3)));
        assert_eq!((link.subscription_period, link.subscription_price), (Some(2_592_000), Some(50)));
        assert_eq!((link.is_primary, link.is_revoked, link.creates_join_request), (false, false, false));

        let raw = base::ExportedChatInvite::ChatInvitePublicJoinRequests;
        assert!(matches!(ChatInviteLink::parse(raw, &Entities::default()), Err(Error::Unexpected(_))));
    }
}
