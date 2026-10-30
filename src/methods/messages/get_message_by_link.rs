use crate::Client;
use crate::errors::{Error, Result};
use crate::types::{Message, PeerRef};
use crate::utils::get_channel_id;

impl Client {
    pub async fn get_message_by_link(&self, link: &str) -> Result<Option<Message>> {
        let (chat, message_id) = parse_link(link).ok_or(Error::InvalidArgument("not a t.me message link"))?;
        Ok(self.get_messages(chat, &[message_id]).await?.pop().flatten())
    }
}

fn parse_link(link: &str) -> Option<(PeerRef, i32)> {
    let link = link.trim();
    let bare = link.strip_prefix("https://").or_else(|| link.strip_prefix("http://")).unwrap_or(link);
    let path = ["t.me/", "telegram.me/", "telegram.dog/"].iter().find_map(|host| bare.strip_prefix(host))?;
    let path = path.split(['?', '#']).next().unwrap_or_default().trim_end_matches('/');
    // t.me/s/<name>/<id> is the same post in the web preview.
    let path = path.strip_prefix("s/").unwrap_or(path);
    let (chat, message_id) = path.rsplit_once('/')?;
    let message_id = message_id.parse().ok().filter(|&id: &i32| id > 0)?;
    let chat = match chat.rsplit_once('/') {
        Some((chat, topic)) if chat != "c" && topic.parse::<i32>().is_ok() => chat,
        _ => chat,
    };
    match chat.split_once('/') {
        Some(("c", channel_id)) => {
            let channel_id = channel_id.parse().ok().filter(|&id: &i64| id > 0)?;
            Some((PeerRef::Id(get_channel_id(channel_id)), message_id))
        }
        // Usernames are at least four of a-z, 0-9 and `_`: that also turns away `t.me/c/<channel>` with no message.
        None if chat.len() >= 4 && chat.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') => {
            Some((PeerRef::Username(chat.into()), message_id))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_link;
    use crate::types::PeerRef;

    #[test]
    fn reads_message_links() {
        let durov = |id| Some((PeerRef::Username("durov".into()), id));
        let channel = |id| Some((PeerRef::Id(-1001234567890), id));
        let cases = [
            ("https://t.me/durov/123", durov(123)),
            ("http://telegram.me/durov/123?single", durov(123)),
            ("t.me/durov/5/123", durov(123)),
            ("https://t.me/durov/123/", durov(123)),
            ("https://t.me/c/1234567890/42", channel(42)),
            ("t.me/c/1234567890/7/42?thread=7", channel(42)),
            ("https://t.me/durov", None),
            ("https://t.me/c/1234567890", None),
            ("https://t.me/+AbC/1", None),
            ("https://t.me//1", None),
            ("https://t.me/s/durov/1", durov(1)),
            ("https://t.me/c/-5/1", None),
            ("https://t.me/c/0/1", None),
            ("https://t.me/durov/0", None),
            ("https://t.me/durov/-3", None),
            ("https://example.com/durov/1", None),
            ("durov/1", None),
        ];
        for (link, expected) in cases {
            assert_eq!(parse_link(link), expected, "{link}");
        }
    }
}
