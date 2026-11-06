use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::MessageEntity;

/// Telegram's terms of service.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct TermsOfService {
    /// Id to pass to [`Client::accept_terms_of_service`](crate::Client::accept_terms_of_service).
    pub id: String,
    /// Text of the terms, without formatting.
    pub text: String,
    /// Formatting of `text`. `None` if Telegram sent no entities.
    pub entities: Option<Vec<MessageEntity>>,
    /// Telegram asks apps to show the terms in a popup.
    pub popup: bool,
    /// Minimum age the user must confirm to have. `None` if no age check is required.
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
