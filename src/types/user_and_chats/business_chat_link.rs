use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::{Chat, MessageEntity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusinessChatLink {
    pub link: String,
    pub text: String,
    pub entities: Option<Vec<MessageEntity>>,
    pub title: Option<String>,
    pub views: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusinessChatLinkInfo {
    pub chat: Chat,
    pub text: String,
    pub entities: Option<Vec<MessageEntity>>,
}

impl BusinessChatLink {
    pub(crate) fn parse(raw: base::BusinessChatLink, entities: &Entities) -> BusinessChatLink {
        let base::BusinessChatLink::BusinessChatLink(l) = raw;
        let types::BusinessChatLink { link, message, entities: raw_entities, title, views } = *l;
        let entities = MessageEntity::parse_all(raw_entities.as_deref().unwrap_or_default(), entities);
        BusinessChatLink { link, text: message, entities, title, views }
    }

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
