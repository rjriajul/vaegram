use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::MessageEntity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslatedText {
    pub text: String,
    pub entities: Option<Vec<MessageEntity>>,
}

impl TranslatedText {
    pub(crate) fn parse_all(result: base::messages::TranslatedText) -> Vec<TranslatedText> {
        let base::messages::TranslatedText::TranslateResult(result) = result;
        let entities = Entities::default();
        result
            .result
            .into_iter()
            .map(|base::TextWithEntities::TextWithEntities(t)| {
                let types::TextWithEntities { text, entities: raw } = *t;
                TranslatedText { text, entities: MessageEntity::parse_all(&raw, &entities) }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::TranslatedText;
    use crate::enums::MessageEntityType;
    use crate::raw::{base, types};

    fn text(text: &str, entities: Vec<base::MessageEntity>) -> base::TextWithEntities {
        base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities { text: text.into(), entities }))
    }

    #[test]
    fn keeps_order_and_entities() {
        let bold = base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 4 });
        let result = vec![text("Hola", vec![bold]), text("mundo", Vec::new())];
        let result = types::messages::TranslateResult { result };
        let parsed = TranslatedText::parse_all(base::messages::TranslatedText::TranslateResult(Box::new(result)));
        let shape: Vec<_> =
            parsed.iter().map(|t| (t.text.as_str(), t.entities.as_ref().map(|e| (e[0].kind, e[0].length)))).collect();
        assert_eq!(shape, [("Hola", Some((MessageEntityType::Bold, 4))), ("mundo", None)]);
    }
}
