use crate::raw::base;
use crate::types::FormattedText;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedText {
    pub text: FormattedText,
    /// The changes against the input, marked with `DiffInsert`, `DiffReplace` and `DiffDelete` entities.
    pub diff: Option<FormattedText>,
}

impl ComposedText {
    pub(crate) fn parse(raw: base::messages::ComposedMessageWithAI) -> ComposedText {
        let base::messages::ComposedMessageWithAI::ComposedMessageWithAI(c) = raw;
        ComposedText { text: FormattedText::parse(c.result_text), diff: c.diff_text.map(FormattedText::parse) }
    }
}

#[cfg(test)]
mod tests {
    use super::ComposedText;
    use crate::enums::MessageEntityType;
    use crate::raw::{base, types};

    fn text(text: &str, entities: Vec<base::MessageEntity>) -> base::TextWithEntities {
        base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities { text: text.into(), entities }))
    }

    fn composed(diff_text: Option<base::TextWithEntities>) -> ComposedText {
        let raw = types::messages::ComposedMessageWithAI { result_text: text("This is a test", Vec::new()), diff_text };
        ComposedText::parse(base::messages::ComposedMessageWithAI::ComposedMessageWithAI(Box::new(raw)))
    }

    #[test]
    fn composed_text_keeps_the_diff() {
        let replace = types::MessageEntityDiffReplace { offset: 10, length: 4, old_text: "tset".into() };
        let replace = base::MessageEntity::MessageEntityDiffReplace(Box::new(replace));
        let parsed = composed(Some(text("This is a test", vec![replace])));
        assert_eq!(parsed.text.text, "This is a test");
        let entities = parsed.diff.and_then(|d| d.entities).expect("the diff was sent");
        assert_eq!((entities[0].kind, entities[0].old_text.as_deref()), (MessageEntityType::DiffReplace, Some("tset")));
    }

    #[test]
    fn composed_text_without_a_diff() {
        // Telegram sends no diff when nothing was corrected.
        assert_eq!(composed(None).diff, None);
    }
}
