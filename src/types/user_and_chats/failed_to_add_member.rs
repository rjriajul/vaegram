use crate::raw::base;

/// A user who wasn't added to a chat, and whether Premium would have changed that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct FailedToAddMember {
    /// Id of the user who wasn't added.
    pub user_id: i64,
    /// Whether the user would have been added if the account had Premium.
    pub premium_would_allow_invite: bool,
    /// Whether the user only receives messages from Premium accounts.
    pub premium_required_to_send_messages: bool,
}

impl FailedToAddMember {
    pub(crate) fn parse(raw: &base::MissingInvitee) -> FailedToAddMember {
        let base::MissingInvitee::MissingInvitee(m) = raw;
        FailedToAddMember {
            user_id: m.user_id,
            premium_would_allow_invite: m.premium_would_allow_invite,
            premium_required_to_send_messages: m.premium_required_for_pm,
        }
    }
}
