use crate::raw::base;

/// What a user asks of you before you can write to them first.
///
/// Returned by [`Client::get_requirements_to_contact`](crate::Client::get_requirements_to_contact).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum ContactRequirement {
    /// Nothing: you can message them.
    Unrestricted,
    /// They accept first messages only from Premium users, and you aren't one.
    Premium,
    /// They charge Stars for a first message.
    PaidMessages {
        /// Stars each message costs.
        stars: i64,
    },
}

impl ContactRequirement {
    pub(crate) fn parse(raw: base::RequirementToContact) -> ContactRequirement {
        match raw {
            base::RequirementToContact::RequirementToContactEmpty => ContactRequirement::Unrestricted,
            base::RequirementToContact::RequirementToContactPremium => ContactRequirement::Premium,
            base::RequirementToContact::RequirementToContactPaidMessages(r) => {
                ContactRequirement::PaidMessages { stars: r.stars_amount }
            }
        }
    }
}
