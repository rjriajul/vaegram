use crate::parser::{Kind, MAX_NESTING, Span};
use crate::raw::{base, types};
use crate::types::Text;

/// Formatting comes from [`Text`]; the methods here add what only rich messages have.
#[derive(Debug, Clone, PartialEq)]
pub struct RichText(pub(crate) base::RichText);

impl RichText {
    pub fn math(source: impl Into<String>) -> Self {
        RichText(base::RichText::TextMath(Box::new(types::TextMath { source: source.into() })))
    }

    pub fn marked(self) -> Self {
        RichText(base::RichText::TextMarked(Box::new(types::TextMarked { text: self.0 })))
    }

    pub fn sub(self) -> Self {
        RichText(base::RichText::TextSubscript(Box::new(types::TextSubscript { text: self.0 })))
    }

    pub fn sup(self) -> Self {
        RichText(base::RichText::TextSuperscript(Box::new(types::TextSuperscript { text: self.0 })))
    }

    pub fn anchor(self, name: impl Into<String>) -> Self {
        RichText(base::RichText::TextAnchor(Box::new(types::TextAnchor { text: self.0, name: name.into() })))
    }
}

impl From<String> for RichText {
    fn from(text: String) -> Self {
        RichText(plain(text))
    }
}

impl From<&str> for RichText {
    fn from(text: &str) -> Self {
        RichText(plain(text.into()))
    }
}

impl From<&String> for RichText {
    fn from(text: &String) -> Self {
        RichText(plain(text.clone()))
    }
}

/// Rich text has no code language or quote span: `pre` drops its language and blockquotes keep only their text.
impl From<Text> for RichText {
    fn from(text: Text) -> Self {
        let units: Vec<u16> = text.text.encode_utf16().collect();
        let mut spans: Vec<(usize, &Span)> = text.spans.iter().enumerate().filter(|(_, s)| s.end > s.start).collect();
        // Spans covering the same range nest in the order they were added: the later one wraps.
        spans.sort_by_key(|&(i, s)| (s.start, -s.end, std::cmp::Reverse(i)));
        let spans: Vec<_> = spans.into_iter().map(|(_, s)| (&s.kind, s.start, s.end)).collect();
        RichText(tree(&units, &spans, 0, text.len, 0))
    }
}

impl<T: Into<RichText>> From<Vec<T>> for RichText {
    fn from(texts: Vec<T>) -> Self {
        let texts = texts.into_iter().map(|t| t.into().0).collect();
        RichText(base::RichText::TextConcat(Box::new(types::TextConcat { texts })))
    }
}

fn plain(text: String) -> base::RichText {
    if text.is_empty() {
        base::RichText::TextEmpty
    } else {
        base::RichText::TextPlain(Box::new(types::TextPlain { text }))
    }
}

// Spans can cross (a quote running into bold): the part of a span past its parent's end is emitted again after the
// parent, so every unit of text appears once. Formatting nested deeper than MAX_NESTING is dropped.
fn tree(units: &[u16], spans: &[(&Kind, i32, i32)], from: i32, to: i32, depth: usize) -> base::RichText {
    let piece = |a: i32, b: i32| String::from_utf16_lossy(&units[a as usize..b as usize]);
    let mut parts = Vec::new();
    let mut at = from;
    if depth < MAX_NESTING {
        for (i, &(kind, start, end)) in spans.iter().enumerate() {
            let (start, end) = (start.max(at), end.min(to));
            if start >= end {
                continue;
            }
            if at < start {
                parts.push(plain(piece(at, start)));
            }
            let inner = &spans[i + 1..];
            let inner = &inner[..inner.iter().take_while(|s| s.1 < end).count()];
            parts.push(wrap(kind, tree(units, inner, start, end, depth + 1), || piece(start, end)));
            at = end;
        }
    }
    if at < to {
        parts.push(plain(piece(at, to)));
    }
    match parts.len() {
        0 => base::RichText::TextEmpty,
        1 => parts.pop().unwrap(),
        _ => base::RichText::TextConcat(Box::new(types::TextConcat { texts: parts })),
    }
}

fn wrap(kind: &Kind, text: base::RichText, source: impl FnOnce() -> String) -> base::RichText {
    use base::RichText as T;
    match kind {
        Kind::Bold => T::TextBold(Box::new(types::TextBold { text })),
        Kind::Italic => T::TextItalic(Box::new(types::TextItalic { text })),
        Kind::Underline => T::TextUnderline(Box::new(types::TextUnderline { text })),
        Kind::Strike => T::TextStrike(Box::new(types::TextStrike { text })),
        Kind::Spoiler => T::TextSpoiler(Box::new(types::TextSpoiler { text })),
        Kind::Code | Kind::Pre(_) => T::TextFixed(Box::new(types::TextFixed { text })),
        Kind::Link(url) => T::TextUrl(Box::new(types::TextUrl { text, url: url.clone(), webpage_id: 0 })),
        &Kind::Mention(user_id) => T::TextMentionName(Box::new(types::TextMentionName { text, user_id })),
        Kind::Blockquote(_) => text,
        &Kind::CustomEmoji(document_id) => {
            T::TextCustomEmoji(Box::new(types::TextCustomEmoji { document_id, alt: source() }))
        }
        &Kind::DateTime(date, flags) => {
            let has = |bit: u8| flags & (1 << bit) != 0;
            T::TextDate(Box::new(types::TextDate {
                relative: has(0),
                short_time: has(1),
                long_time: has(2),
                short_date: has(3),
                long_date: has(4),
                day_of_week: has(5),
                text,
                date,
            }))
        }
    }
}

pub(crate) fn push_plain(text: &base::RichText, out: &mut String) {
    use base::RichText as T;
    match text {
        T::TextPlain(t) => out.push_str(&t.text),
        T::TextConcat(t) => t.texts.iter().for_each(|t| push_plain(t, out)),
        T::TextMath(t) => out.push_str(&t.source),
        T::TextCustomEmoji(t) => out.push_str(&t.alt),
        _ => {
            if let Some(inner) = inner(text) {
                push_plain(inner, out);
            }
        }
    }
}

pub(crate) fn push_mentions(text: &base::RichText, out: &mut Vec<i64>) {
    match text {
        base::RichText::TextMentionName(t) => {
            out.push(t.user_id);
            push_mentions(&t.text, out);
        }
        base::RichText::TextConcat(t) => t.texts.iter().for_each(|t| push_mentions(t, out)),
        _ => {
            if let Some(inner) = inner(text) {
                push_mentions(inner, out);
            }
        }
    }
}

fn inner(text: &base::RichText) -> Option<&base::RichText> {
    use base::RichText as T;
    Some(match text {
        T::TextEmpty
        | T::TextPlain(_)
        | T::TextConcat(_)
        | T::TextImage(_)
        | T::TextMath(_)
        | T::TextCustomEmoji(_) => return None,
        T::TextBold(t) => &t.text,
        T::TextItalic(t) => &t.text,
        T::TextUnderline(t) => &t.text,
        T::TextStrike(t) => &t.text,
        T::TextFixed(t) => &t.text,
        T::TextUrl(t) => &t.text,
        T::TextEmail(t) => &t.text,
        T::TextSubscript(t) => &t.text,
        T::TextSuperscript(t) => &t.text,
        T::TextMarked(t) => &t.text,
        T::TextPhone(t) => &t.text,
        T::TextAnchor(t) => &t.text,
        T::TextSpoiler(t) => &t.text,
        T::TextMention(t) => &t.text,
        T::TextHashtag(t) => &t.text,
        T::TextBotCommand(t) => &t.text,
        T::TextCashtag(t) => &t.text,
        T::TextAutoUrl(t) => &t.text,
        T::TextAutoEmail(t) => &t.text,
        T::TextAutoPhone(t) => &t.text,
        T::TextBankCard(t) => &t.text,
        T::TextMentionName(t) => &t.text,
        T::TextDate(t) => &t.text,
        T::TextDiff(t) => &t.text,
        T::TextButton(t) => &t.text,
        T::TextTonAddress(t) => &t.text,
    })
}

#[cfg(test)]
mod tests {
    use super::{RichText, inner, push_plain};
    use crate::parser::MAX_NESTING;
    use crate::raw::{base, types};
    use crate::types::Text;

    fn plain(text: &str) -> base::RichText {
        base::RichText::TextPlain(Box::new(types::TextPlain { text: text.into() }))
    }

    fn bold(text: base::RichText) -> base::RichText {
        base::RichText::TextBold(Box::new(types::TextBold { text }))
    }

    fn italic(text: base::RichText) -> base::RichText {
        base::RichText::TextItalic(Box::new(types::TextItalic { text }))
    }

    fn concat(texts: Vec<base::RichText>) -> base::RichText {
        base::RichText::TextConcat(Box::new(types::TextConcat { texts }))
    }

    #[test]
    fn text_entities_become_a_tree() {
        assert_eq!(RichText::from("").0, base::RichText::TextEmpty);
        assert_eq!(RichText::from(Text::new()).0, base::RichText::TextEmpty);
        assert_eq!(RichText::from(Text::from("<b>x</b>")).0, plain("<b>x</b>"));
        let t = Text::new().plain("😀 ").bold(Text::new().plain("a").italic("𝕏")).plain("!");
        let expected = concat(vec![plain("😀 "), bold(concat(vec![plain("a"), italic(plain("𝕏"))])), plain("!")]);
        assert_eq!(RichText::from(t).0, expected);
        assert_eq!(RichText::from(Text::new().bold("x")).0, bold(plain("x")));
    }

    #[test]
    fn crossing_markdown_builds_a_nested_tree() {
        let t = Text::from_markdown("**ab __cd** ef__");
        let expected = concat(vec![bold(concat(vec![plain("ab "), italic(plain("cd"))])), italic(plain(" ef"))]);
        assert_eq!(RichText::from(t).0, expected);
    }

    #[test]
    fn a_quote_crossing_bold_keeps_each_character_once() {
        let mut out = String::new();
        push_plain(&RichText::from(Text::from_markdown("**a\n> b** c\n> d")).0, &mut out);
        assert_eq!(out, "a\nb c\nd");
    }

    #[test]
    fn nesting_stops_at_the_cap() {
        let mut t = Text::new().plain("x");
        for _ in 0..100 {
            t = Text::new().bold(t);
        }
        let rich = RichText::from(t).0;
        let (mut depth, mut at) = (0, &rich);
        while let Some(next) = inner(at) {
            (depth, at) = (depth + 1, next);
        }
        assert_eq!((depth, at), (MAX_NESTING, &plain("x")));
    }

    #[test]
    fn rich_only_formats_and_kinds_map_to_raw() {
        let t =
            Text::new().date_time("soon", 100, "wDt").custom_emoji("👍", 9).link(Text::new().code("x"), "https://t.me");
        let base::RichText::TextConcat(c) = RichText::from(t).0 else { panic!("expected a concat") };
        let base::RichText::TextDate(d) = &c.texts[0] else { panic!("expected a date") };
        assert_eq!(
            (d.relative, d.day_of_week, d.long_date, d.short_date, d.short_time, d.long_time, d.date),
            (false, true, true, false, true, false, 100)
        );
        let emoji = types::TextCustomEmoji { document_id: 9, alt: "👍".into() };
        assert_eq!(c.texts[1], base::RichText::TextCustomEmoji(Box::new(emoji)));
        let base::RichText::TextUrl(u) = &c.texts[2] else { panic!("expected a link") };
        assert_eq!((u.url.as_str(), u.webpage_id), ("https://t.me", 0));
        assert!(matches!(&u.text, base::RichText::TextFixed(_)));

        let h2o = RichText::from(vec![RichText::from("H"), RichText::from("2").sub(), "O".into()]);
        let base::RichText::TextConcat(c) = h2o.0 else { panic!("expected a concat") };
        assert!(matches!(&c.texts[1], base::RichText::TextSubscript(_)));
        assert!(matches!(RichText::from("x").marked().sup().0, base::RichText::TextSuperscript(_)));
        let base::RichText::TextAnchor(a) = RichText::from("").anchor("top").0 else { panic!("expected an anchor") };
        assert_eq!((a.text, a.name.as_str()), (base::RichText::TextEmpty, "top"));
    }
}
