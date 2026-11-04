use crate::types::{Chat, Message, User};
use crate::utils::get_channel_id;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerRef {
    Id(i64),
    Me,
    Username(Box<str>),
    Invite(Box<str>),
}

impl From<i64> for PeerRef {
    fn from(id: i64) -> PeerRef {
        PeerRef::Id(id)
    }
}

impl From<&str> for PeerRef {
    fn from(s: &str) -> PeerRef {
        parse(s.trim())
    }
}

impl From<&String> for PeerRef {
    fn from(s: &String) -> PeerRef {
        parse(s.trim())
    }
}

impl From<String> for PeerRef {
    fn from(s: String) -> PeerRef {
        parse(s.trim())
    }
}

impl From<&Chat> for PeerRef {
    fn from(chat: &Chat) -> PeerRef {
        PeerRef::Id(chat.id)
    }
}

impl From<&User> for PeerRef {
    fn from(user: &User) -> PeerRef {
        PeerRef::Id(user.id)
    }
}

impl From<&Message> for PeerRef {
    fn from(message: &Message) -> PeerRef {
        PeerRef::Id(message.chat.id)
    }
}

fn parse(s: &str) -> PeerRef {
    // `i64` parsing accepts a leading `+`, which here starts an invite hash.
    if let (false, Ok(id)) = (s.starts_with('+'), s.parse()) {
        return PeerRef::Id(id);
    }
    if let Some(domain) = PeerRef::tg_arg(s, "resolve", "domain") {
        return PeerRef::Username(domain.into());
    }
    if let Some(hash) = PeerRef::tg_arg(s, "join", "invite") {
        return PeerRef::Invite(hash.into());
    }
    let Some(path) = PeerRef::link_path(s) else {
        if s.eq_ignore_ascii_case("me") || s.eq_ignore_ascii_case("self") {
            return PeerRef::Me;
        }
        return name(s.strip_prefix('@').unwrap_or(s));
    };
    let (head, tail) = path.split_once('/').unwrap_or((path, ""));
    let tail = tail.split('/').next().unwrap_or_default();
    match (head, tail.parse()) {
        ("c", Ok(channel_id)) => PeerRef::Id(get_channel_id(channel_id)),
        ("joinchat", _) => PeerRef::Invite(tail.into()),
        _ => name(head),
    }
}

impl PeerRef {
    /// The path of a `t.me` link (or one of its mirrors), without the scheme, host, query or fragment.
    pub(crate) fn link_path(s: &str) -> Option<&str> {
        let bare = strip_prefix_ci(s, "https://").or_else(|| strip_prefix_ci(s, "http://")).unwrap_or(s);
        let bare = strip_prefix_ci(bare, "www.").unwrap_or(bare);
        let path = ["t.me/", "telegram.me/", "telegram.dog/"].iter().find_map(|host| strip_prefix_ci(bare, host))?;
        path.split(['?', '#']).next()
    }

    /// The non-empty `key` argument of a `tg://<action>?…` or `tg:<action>?…` link.
    pub(crate) fn tg_arg<'a>(s: &'a str, action: &str, key: &str) -> Option<&'a str> {
        let rest = strip_prefix_ci(s, "tg:")?;
        let (head, query) = rest.strip_prefix("//").unwrap_or(rest).split_once('?')?;
        if !head.trim_end_matches('/').eq_ignore_ascii_case(action) {
            return None;
        }
        let query = query.split('#').next().unwrap_or_default();
        query.split('&').find_map(|pair| pair.strip_prefix(key)?.strip_prefix('=')).filter(|value| !value.is_empty())
    }
}

/// Schemes and hosts are case-insensitive; the rest of a link isn't.
fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let (head, rest) = (s.get(..prefix.len())?, s.get(prefix.len()..)?);
    head.eq_ignore_ascii_case(prefix).then_some(rest)
}

fn name(s: &str) -> PeerRef {
    match s.strip_prefix('+') {
        Some(hash) => PeerRef::Invite(hash.into()),
        None => PeerRef::Username(s.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::PeerRef;

    #[test]
    fn parses_every_way_to_name_a_chat() {
        let name = |s: &str| PeerRef::Username(s.into());
        let invite = |s: &str| PeerRef::Invite(s.into());
        let cases = [
            ("-1001234567890", PeerRef::Id(-1001234567890)),
            (" 42 ", PeerRef::Id(42)),
            ("me", PeerRef::Me),
            ("Self", PeerRef::Me),
            ("@durov", name("durov")),
            ("durov", name("durov")),
            ("t.me/durov", name("durov")),
            ("https://t.me/durov/123?single", name("durov")),
            ("http://telegram.me/durov", name("durov")),
            ("https://t.me/c/1234567890/5", PeerRef::Id(-1001234567890)),
            ("t.me/c/1234567890", PeerRef::Id(-1001234567890)),
            ("https://t.me/+AbC-d_1", invite("AbC-d_1")),
            ("t.me/joinchat/AbC", invite("AbC")),
            ("+AbC", invite("AbC")),
            ("+123", invite("123")),
            ("t.me/me", name("me")),
        ];
        for (input, expected) in cases {
            assert_eq!(PeerRef::from(input), expected, "{input}");
        }
        assert_eq!(PeerRef::from(String::from("@durov")), name("durov"));
        assert_eq!(PeerRef::from(&String::from("me")), PeerRef::Me);
        assert_eq!(PeerRef::from(-5), PeerRef::Id(-5));
    }

    #[test]
    fn parses_tg_scheme_and_mirror_domains() {
        let name = |s: &str| PeerRef::Username(s.into());
        let invite = |s: &str| PeerRef::Invite(s.into());
        let cases = [
            ("tg://resolve?domain=durov", name("durov")),
            ("tg:resolve?domain=durov&post=5", name("durov")),
            ("tg://resolve/?start=x&domain=durov#top", name("durov")),
            ("tg://join?invite=AbC-d_1", invite("AbC-d_1")),
            ("tg:join?invite=AbC", invite("AbC")),
            ("https://telegram.dog/durov", name("durov")),
            ("telegram.dog/joinchat/AbC", invite("AbC")),
            ("https://www.t.me/durov", name("durov")),
            ("www.telegram.me/+AbC", invite("AbC")),
            ("https://telegram.dog/c/1234567890/5", PeerRef::Id(-1001234567890)),
            ("HTTPS://T.ME/Durov", name("Durov")),
            ("Http://WWW.Telegram.Dog/+AbC", invite("AbC")),
            ("TG://Resolve?domain=durov", name("durov")),
            ("Tg:JOIN?invite=AbC", invite("AbC")),
            ("tg://resolve?Domain=durov", name("tg://resolve?Domain=durov")),
            ("ftp://t.me/durov", name("ftp://t.me/durov")),
            // Links with no usable argument, and unknown actions, are read as before: as a name.
            ("tg://resolve", name("tg://resolve")),
            ("tg://resolve?domain=", name("tg://resolve?domain=")),
            ("tg://resolve?domains=durov", name("tg://resolve?domains=durov")),
            ("tg://join?hash=AbC", name("tg://join?hash=AbC")),
            ("tg://settings", name("tg://settings")),
        ];
        for (input, expected) in cases {
            assert_eq!(PeerRef::from(input), expected, "{input}");
        }
    }
}
