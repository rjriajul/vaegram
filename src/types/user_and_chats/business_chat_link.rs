use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::{Chat, MessageEntity};

/// A link that opens a chat with a business account, with a prefilled message.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BusinessChatLink {
    /// The full `t.me` link.
    pub link: String,
    /// Text prefilled in the message box.
    pub text: String,
    /// Formatting of the prefilled text. `None` if the text is plain.
    pub entities: Option<Vec<MessageEntity>>,
    /// Name the owner gave the link. `None` if the link is unnamed.
    pub title: Option<String>,
    /// Times the link has been opened.
    pub views: i32,
}

/// What a business chat link resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BusinessChatLinkInfo {
    /// The business account the link opens.
    pub chat: Chat,
    /// Text prefilled in the message box.
    pub text: String,
    /// Formatting of the prefilled text. `None` if the text is plain.
    pub entities: Option<Vec<MessageEntity>>,
}

impl BusinessChatLink {
    pub(crate) fn parse(raw: base::BusinessChatLink, entities: &Entities) -> BusinessChatLink {
        let base::BusinessChatLink::BusinessChatLink(l) = raw;
        let types::BusinessChatLink { link, message, entities: raw_entities, title, views } = *l;
        let entities = MessageEntity::parse_all(raw_entities.as_deref().unwrap_or_default(), entities);
        BusinessChatLink { link, text: message, entities, title, views }
    }

    /// Returns the last path segment of the link, such as `AbCdEf` for `https://t.me/m/AbCdEf`.
    pub fn slug(&self) -> &str {
        self.link.rsplit('/').next().unwrap_or(&self.link)
    }
}

impl BusinessChatLinkInfo {
    pub(crate) fn parse(raw: types::account::ResolvedBusinessChatLinks, entities: &Entities) -> BusinessChatLinkInfo {
        let chat = Chat::parse(&raw.peer, entities, false);
        let parsed = MessageEntity::parse_all(raw.entities.as_deref().unwrap_or_default(), entities);
        BusinessChatLinkInfo { chat, text: raw.message, entities: parsed }
    }
}

#[cfg(test)]
mod tests {
    use super::BusinessChatLink;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};

    #[test]
    fn parses_links_and_slugs() {
        let bold = base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 2 });
        let raw = types::BusinessChatLink {
            link: "https://t.me/m/AbCdEf".into(),
            message: "hi".into(),
            entities: Some(vec![bold]),
            title: Some("Sales".into()),
            views: 3,
        };
        let entities = Entities::default();
        let l = BusinessChatLink::parse(base::BusinessChatLink::BusinessChatLink(Box::new(raw)), &entities);
        let got = (l.slug(), l.text.as_str(), l.title.as_deref(), l.views, l.entities.as_ref().map_or(0, Vec::len));
        assert_eq!(got, ("AbCdEf", "hi", Some("Sales"), 3, 1));
    }
}
