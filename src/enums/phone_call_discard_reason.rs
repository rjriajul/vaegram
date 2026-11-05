use crate::raw::base;

/// Why a private call ended.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PhoneCallDiscardReason {
    /// Nobody answered.
    Missed,
    /// The connection dropped.
    Disconnected,
    /// A participant hung up.
    HungUp,
    /// The callee was busy.
    Busy,
    /// The call moved to a conference call; holds that call's invite slug.
    MigrateConferenceCall(String),
}

impl PhoneCallDiscardReason {
    pub(crate) fn parse(raw: &base::PhoneCallDiscardReason) -> PhoneCallDiscardReason {
        use base::PhoneCallDiscardReason as R;
        match raw {
            R::PhoneCallDiscardReasonMissed => PhoneCallDiscardReason::Missed,
            R::PhoneCallDiscardReasonDisconnect => PhoneCallDiscardReason::Disconnected,
            R::PhoneCallDiscardReasonHangup => PhoneCallDiscardReason::HungUp,
            R::PhoneCallDiscardReasonBusy => PhoneCallDiscardReason::Busy,
            R::PhoneCallDiscardReasonMigrateConferenceCall(r) => {
                PhoneCallDiscardReason::MigrateConferenceCall(r.slug.clone())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PhoneCallDiscardReason;
    use crate::raw::{base, types};

    #[test]
    fn every_reason_maps() {
        use base::PhoneCallDiscardReason as R;
        let slug = R::PhoneCallDiscardReasonMigrateConferenceCall(Box::new(
            types::PhoneCallDiscardReasonMigrateConferenceCall { slug: "abc".into() },
        ));
        let reasons = [
            R::PhoneCallDiscardReasonMissed,
            R::PhoneCallDiscardReasonDisconnect,
            R::PhoneCallDiscardReasonHangup,
            R::PhoneCallDiscardReasonBusy,
            slug,
        ];
        let parsed: Vec<_> = reasons.iter().map(PhoneCallDiscardReason::parse).collect();
        assert_eq!(
            parsed,
            [
                PhoneCallDiscardReason::Missed,
                PhoneCallDiscardReason::Disconnected,
                PhoneCallDiscardReason::HungUp,
                PhoneCallDiscardReason::Busy,
                PhoneCallDiscardReason::MigrateConferenceCall("abc".into()),
            ]
        );
    }
}
