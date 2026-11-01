use super::text_with_entities;
use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::MessageEntity;

/// An independent fact check attached to a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactCheck {
    /// The text is out of date and is being fetched again.
    pub need_check: bool,
    /// The two-letter code of the country the check is shown in.
    pub country: Option<String>,
    pub text: Option<String>,
    pub text_entities: Option<Vec<MessageEntity>>,
    pub hash: i64,
}

impl FactCheck {
    pub(crate) fn parse(raw: &base::FactCheck, entities: &Entities) -> FactCheck {
        let base::FactCheck::FactCheck(f) = raw;
        let (text, text_entities) = match &f.text {
            Some(t) => {
                let (text, text_entities) = text_with_entities(t, entities);
                (Some(text), text_entities)
            }
            None => (None, None),
        };
        FactCheck { need_check: f.need_check, country: f.country.clone(), text, text_entities, hash: f.hash }
    }
}

#[cfg(test)]
mod tests {
    use super::FactCheck;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};

    fn parse(f: types::FactCheck) -> FactCheck {
        FactCheck::parse(&base::FactCheck::FactCheck(Box::new(f)), &Entities::default())
    }

    #[test]
    fn parses_a_checked_text() {
        let bold = base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 4 });
        let text = types::TextWithEntities { text: "Fake news".into(), entities: vec![bold] };
        let raw = types::FactCheck {
            need_check: false,
            country: Some("DE".into()),
            text: Some(base::TextWithEntities::TextWithEntities(Box::new(text))),
            hash: 77,
        };
        let check = parse(raw);
        assert_eq!((check.country.as_deref(), check.text.as_deref(), check.hash), (Some("DE"), Some("Fake news"), 77));
        assert_eq!(check.text_entities.map(|e| e.len()), Some(1));
    }

    #[test]
    fn a_pending_check_has_no_text() {
        let check = parse(types::FactCheck { need_check: true, country: None, text: None, hash: 1 });
        assert_eq!(check, FactCheck { need_check: true, country: None, text: None, text_entities: None, hash: 1 });
    }
}
