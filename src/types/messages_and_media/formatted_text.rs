use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::MessageEntity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormattedText {
    pub text: String,
    pub entities: Option<Vec<MessageEntity>>,
}

impl FormattedText {
    pub(crate) fn parse(raw: base::TextWithEntities) -> FormattedText {
        let base::TextWithEntities::TextWithEntities(t) = raw;
        let types::TextWithEntities { text, entities } = *t;
        FormattedText { text, entities: MessageEntity::parse_all(&entities, &Entities::default()) }
    }
}

#[cfg(test)]
mod tests {
    use super::FormattedText;
    use crate::enums::MessageEntityType;
    use crate::raw::{base, types};

    #[test]
    fn formatted_text_maps_entities() {
        let text = |entities| {
            let text = types::TextWithEntities { text: "Hola".into(), entities };
            FormattedText::parse(base::TextWithEntities::TextWithEntities(Box::new(text)))
        };
        let bold = base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 4 });
        let parsed = text(vec![bold]);
        let shape = parsed.entities.as_ref().map(|e| (e[0].kind, e[0].offset, e[0].length));
        assert_eq!((parsed.text.as_str(), shape), ("Hola", Some((MessageEntityType::Bold, 0, 4))));
        // No entities is `None`, as on every other text.
        assert_eq!(text(Vec::new()).entities, None);
    }
}
