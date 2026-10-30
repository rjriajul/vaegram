use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::MessageEntity;

#[derive(Debug, Clone, PartialEq)]
pub struct TermsOfService {
    pub id: String,
    pub text: String,
    pub entities: Option<Vec<MessageEntity>>,
    pub popup: bool,
    pub min_age_confirm: Option<i32>,
}

impl TermsOfService {
    pub(crate) fn parse(raw: base::help::TermsOfService) -> TermsOfService {
        let base::help::TermsOfService::TermsOfService(t) = raw;
        let base::DataJSON::DataJSON(id) = t.id;
        TermsOfService {
            id: id.data,
            entities: MessageEntity::parse_all(&t.entities, &Entities::default()),
            text: t.text,
            popup: t.popup,
            min_age_confirm: t.min_age_confirm,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TermsOfService;
    use crate::raw::{base, types};

    #[test]
    fn terms_without_entities_or_age_gate() {
        let id = base::DataJSON::DataJSON(Box::new(types::DataJSON { data: "{}".into() }));
        let raw = types::help::TermsOfService {
            popup: false,
            id,
            text: "Hi".into(),
            entities: Vec::new(),
            min_age_confirm: None,
        };
        let tos = TermsOfService::parse(base::help::TermsOfService::TermsOfService(Box::new(raw)));
        assert_eq!((tos.entities, tos.min_age_confirm, tos.popup), (None, None, false));
    }
}
