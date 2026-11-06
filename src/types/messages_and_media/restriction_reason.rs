use crate::raw::base;

/// Why a chat, user or message is hidden on some platforms.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RestrictionReason {
    /// The platforms it applies to, such as `ios` or `all`.
    pub platform: String,
    /// Short code for the reason, such as `sensitive`.
    pub reason: String,
    /// The text shown in place of the content.
    pub text: String,
}

impl RestrictionReason {
    pub(crate) fn parse(raw: &base::RestrictionReason) -> RestrictionReason {
        let base::RestrictionReason::RestrictionReason(r) = raw;
        RestrictionReason { platform: r.platform.clone(), reason: r.reason.clone(), text: r.text.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::RestrictionReason;
    use crate::raw::{base, types};

    #[test]
    fn parses_platform_reason_and_text() {
        let raw =
            types::RestrictionReason { platform: "ios".into(), reason: "sensitive".into(), text: "hidden".into() };
        let parsed = RestrictionReason::parse(&base::RestrictionReason::RestrictionReason(raw.into()));
        let expected = RestrictionReason { platform: "ios".into(), reason: "sensitive".into(), text: "hidden".into() };
        assert_eq!(parsed, expected);
    }
}
