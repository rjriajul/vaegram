use std::sync::PoisonError;

use crate::Client;
use crate::enums::ParseMode;

impl Client {
    /// Changes the markup every later text and caption is read with, unless a call sets its own `parse_mode`.
    /// Calls already in flight keep the mode they started with.
    pub fn set_parse_mode(&self, mode: ParseMode) {
        *self.0.parse_mode.lock().unwrap_or_else(PoisonError::into_inner) = mode;
    }

    /// The markup in force now: what the builder set, or the last `set_parse_mode`.
    pub(crate) fn parse_mode(&self) -> ParseMode {
        *self.0.parse_mode.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::ParseMode;

    #[tokio::test]
    async fn set_parse_mode_changes_later_sends() {
        let client = Client::offline();
        let mut seen = Vec::new();
        for mode in [ParseMode::Disabled, ParseMode::Html, ParseMode::Disabled] {
            client.set_parse_mode(mode);
            assert_eq!(client.parse_mode(), mode);
            let (text, entities) = client.write_text("<b>x</b>".into(), None).await.unwrap();
            seen.push((text, entities.len()));
        }
        let literal = ("<b>x</b>".to_string(), 0);
        assert_eq!(seen, [literal.clone(), ("x".to_string(), 1), literal]);
    }

    #[tokio::test]
    async fn a_calls_own_mode_wins() {
        let client = Client::offline();
        client.set_parse_mode(ParseMode::Html);
        let (text, entities) = client.write_text("<b>x</b>".into(), Some(ParseMode::Disabled)).await.unwrap();
        assert_eq!((text.as_str(), entities.len()), ("<b>x</b>", 0));
    }
}
