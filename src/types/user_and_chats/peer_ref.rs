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
    let bare = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://")).unwrap_or(s);
    let Some(path) = bare.strip_prefix("t.me/").or_else(|| bare.strip_prefix("telegram.me/")) else {
        if s.eq_ignore_ascii_case("me") || s.eq_ignore_ascii_case("self") {
            return PeerRef::Me;
        }
        return name(s.strip_prefix('@').unwrap_or(s));
    };
    let path = path.split(['?', '#']).next().unwrap_or_default();
    let (head, tail) = path.split_once('/').unwrap_or((path, ""));
    let tail = tail.split('/').next().unwrap_or_default();
    match (head, tail.parse()) {
        ("c", Ok(channel_id)) => PeerRef::Id(get_channel_id(channel_id)),
        ("joinchat", _) => PeerRef::Invite(tail.into()),
        _ => name(head),
    }
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
}
