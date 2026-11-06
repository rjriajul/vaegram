use crate::raw::base;

/// Why a chat is reported, for [`Client::report_chat`](crate::Client::report_chat).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ReportReason {
    /// Spam.
    Spam,
    /// Violence.
    Violence,
    /// Pornography.
    Pornography,
    /// Child abuse.
    ChildAbuse,
    /// Any reason not listed here.
    Other,
    /// Copyright infringement.
    Copyright,
    /// A location-based group that has nothing to do with its location.
    GeoIrrelevant,
    /// An account or chat that impersonates someone else.
    Fake,
    /// Illegal drugs.
    IllegalDrugs,
    /// Personal details shared without consent.
    PersonalDetails,
}

impl ReportReason {
    pub(crate) fn write(self) -> base::ReportReason {
        use base::ReportReason as R;
        match self {
            Self::Spam => R::InputReportReasonSpam,
            Self::Violence => R::InputReportReasonViolence,
            Self::Pornography => R::InputReportReasonPornography,
            Self::ChildAbuse => R::InputReportReasonChildAbuse,
            Self::Other => R::InputReportReasonOther,
            Self::Copyright => R::InputReportReasonCopyright,
            Self::GeoIrrelevant => R::InputReportReasonGeoIrrelevant,
            Self::Fake => R::InputReportReasonFake,
            Self::IllegalDrugs => R::InputReportReasonIllegalDrugs,
            Self::PersonalDetails => R::InputReportReasonPersonalDetails,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ReportReason;
    use crate::raw::Serializable;

    #[test]
    fn each_reason_maps_to_its_constructor() {
        use ReportReason::*;
        let reasons = [
            (Spam, 0x58dbcab8_u32),
            (Violence, 0x1e22c78d),
            (Pornography, 0x2e59d922),
            (ChildAbuse, 0xadf44ee3),
            (Other, 0xc1e4a2b1),
            (Copyright, 0x9b89f93a),
            (GeoIrrelevant, 0xdbd4feed),
            (Fake, 0xf5ddd6e7),
            (IllegalDrugs, 0x0a8eb2be),
            (PersonalDetails, 0x9ec7863d),
        ];
        for (reason, constructor) in reasons {
            assert_eq!(reason.write().to_bytes(), constructor.to_le_bytes(), "{reason:?}");
        }
    }
}
