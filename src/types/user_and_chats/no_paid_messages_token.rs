use crate::raw::base;

/// A link that lets whoever opens it message you without paying, even if you charge for messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoPaidMessagesToken {
    pub token: String,
    pub link: String,
    pub expiration_date: Option<i32>,
}

impl NoPaidMessagesToken {
    pub(crate) fn parse(raw: base::ExportedNoPaidMessagesToken) -> NoPaidMessagesToken {
        let base::ExportedNoPaidMessagesToken::ExportedNoPaidMessagesToken(t) = raw;
        let t = *t;
        NoPaidMessagesToken { token: t.token, link: t.url, expiration_date: t.expires }
    }
}
