use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::Chat;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromoData {
    pub chat: Option<Chat>,
    pub proxy: bool,
    pub psa_type: Option<String>,
    pub psa_message: Option<String>,
    pub expires: i32,
    pub pending_suggestions: Vec<String>,
    pub dismissed_suggestions: Vec<String>,
    pub custom_pending_suggestion: Option<PendingSuggestion>,
}

/// Title and description arrive as formatted text; only the plain text is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSuggestion {
    pub suggestion: String,
    pub title: String,
    pub description: String,
    pub url: String,
}

impl PromoData {
    pub(crate) fn parse(raw: types::help::PromoData, entities: &Entities) -> PromoData {
        let plain = |base::TextWithEntities::TextWithEntities(t): base::TextWithEntities| t.text;
        PromoData {
            chat: raw.peer.as_ref().map(|p| Chat::parse(p, entities, false)),
            proxy: raw.proxy,
            psa_type: raw.psa_type,
            psa_message: raw.psa_message,
            expires: raw.expires,
            pending_suggestions: raw.pending_suggestions,
            dismissed_suggestions: raw.dismissed_suggestions,
            custom_pending_suggestion: raw.custom_pending_suggestion.map(
                |base::PendingSuggestion::PendingSuggestion(s)| {
                    let s = *s;
                    PendingSuggestion {
                        suggestion: s.suggestion,
                        title: plain(s.title),
                        description: plain(s.description),
                        url: s.url,
                    }
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PendingSuggestion, PromoData};
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};

    fn promo(custom: Option<base::PendingSuggestion>) -> types::help::PromoData {
        types::help::PromoData {
            proxy: false,
            expires: 5,
            peer: None,
            psa_type: None,
            psa_message: None,
            pending_suggestions: Vec::new(),
            dismissed_suggestions: Vec::new(),
            custom_pending_suggestion: custom,
            chats: Vec::new(),
            users: Vec::new(),
        }
    }

    fn text(text: &str) -> base::TextWithEntities {
        base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities {
            text: text.into(),
            entities: Vec::new(),
        }))
    }

    #[test]
    fn keeps_the_custom_pending_suggestion() {
        let custom = types::PendingSuggestion {
            suggestion: "UPGRADE".into(),
            title: text("Upgrade"),
            description: text("Get more"),
            url: "https://t.me/x".into(),
        };
        let raw = promo(Some(base::PendingSuggestion::PendingSuggestion(Box::new(custom))));
        let suggestion = PromoData::parse(raw, &Entities::default()).custom_pending_suggestion;
        let expected = PendingSuggestion {
            suggestion: "UPGRADE".into(),
            title: "Upgrade".into(),
            description: "Get more".into(),
            url: "https://t.me/x".into(),
        };
        assert_eq!(suggestion, Some(expected));
    }

    #[test]
    fn a_promo_without_a_peer_or_suggestion_parses_empty() {
        let parsed = PromoData::parse(promo(None), &Entities::default());
        assert_eq!((parsed.chat, parsed.custom_pending_suggestion, parsed.expires), (None, None, 5));
    }
}
