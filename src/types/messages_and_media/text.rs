use std::ops::{Bound, RangeBounds};

use crate::dispatcher::Entities;
use crate::enums::ParseMode;
use crate::parser::{self, Kind, Span};
use crate::raw::base;
use crate::types::MessageEntity;

/// A parse mode only applies to a `Text` made straight from a string; a built or parsed one is sent as is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Text {
    pub(crate) text: String,
    pub(crate) len: i32,
    pub(crate) spans: Vec<Span>,
    unparsed: bool,
}

impl Text {
    pub fn new() -> Self {
        Text::default()
    }

    pub fn from_html(html: &str) -> Self {
        parser::parse(html, ParseMode::Html)
    }

    pub fn from_markdown(markdown: &str) -> Self {
        parser::parse(markdown, ParseMode::Markdown)
    }

    /// A `Text` made straight from a string comes out as plain text: it was never parsed.
    pub fn to_html(&self) -> String {
        parser::unparse(self, ParseMode::Html)
    }

    /// Formatting Markdown can't express (custom emoji, dates, quotes not covering whole lines) is left out.
    pub fn to_markdown(&self) -> String {
        parser::unparse(self, ParseMode::Markdown)
    }

    pub(crate) fn from_raw(text: &str, entities: &[base::MessageEntity]) -> Text {
        let spans = entities.iter().filter_map(Span::read).collect();
        Text { text: text.into(), len: text.encode_utf16().count() as i32, spans, unparsed: false }
    }

    pub fn plain(mut self, text: impl Into<Text>) -> Self {
        self.append(text.into());
        self
    }

    pub fn bold(self, text: impl Into<Text>) -> Self {
        self.wrap(Kind::Bold, text)
    }

    pub fn italic(self, text: impl Into<Text>) -> Self {
        self.wrap(Kind::Italic, text)
    }

    pub fn underline(self, text: impl Into<Text>) -> Self {
        self.wrap(Kind::Underline, text)
    }

    pub fn strike(self, text: impl Into<Text>) -> Self {
        self.wrap(Kind::Strike, text)
    }

    pub fn spoiler(self, text: impl Into<Text>) -> Self {
        self.wrap(Kind::Spoiler, text)
    }

    pub fn code(self, text: impl Into<Text>) -> Self {
        self.wrap(Kind::Code, text)
    }

    pub fn pre(self, text: impl Into<Text>, language: impl Into<String>) -> Self {
        self.wrap(Kind::Pre(language.into()), text)
    }

    pub fn link(self, text: impl Into<Text>, url: impl Into<String>) -> Self {
        self.wrap(Kind::Link(url.into()), text)
    }

    pub fn mention(self, text: impl Into<Text>, user_id: i64) -> Self {
        self.wrap(Kind::Mention(user_id), text)
    }

    pub fn custom_emoji(self, emoji: impl Into<Text>, document_id: i64) -> Self {
        self.wrap(Kind::CustomEmoji(document_id), emoji)
    }

    pub fn date_time(self, text: impl Into<Text>, unix_time: i32, format: &str) -> Self {
        self.wrap(Kind::date_time(unix_time, format), text)
    }

    pub fn blockquote(self, text: impl Into<Text>, expandable: bool) -> Self {
        self.wrap(Kind::Blockquote(expandable), text)
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Counts UTF-16 units, as Telegram does for entity offsets and the 4096 limit.
    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn entities(&self) -> Vec<MessageEntity> {
        self.write_entities().iter().map(|e| MessageEntity::parse(e, &Entities::default())).collect()
    }

    /// An offset inside a surrogate pair moves back to the pair's start; a reversed range gives an empty `Text`.
    pub fn slice(&self, range: impl RangeBounds<usize>) -> Text {
        let unit = |n: usize| n.min(self.len()) as i32;
        let start = match range.start_bound() {
            Bound::Included(&n) => unit(n),
            Bound::Excluded(&n) => unit(n.saturating_add(1)),
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&n) => unit(n.saturating_add(1)),
            Bound::Excluded(&n) => unit(n),
            Bound::Unbounded => self.len,
        };
        let from = self.boundary((0, 0), start);
        self.cut(from, self.boundary(from, end))
    }

    /// Cuts after a line break, else a space, in each part's second half; never inside a custom emoji or date.
    pub fn split(&self, max: usize) -> Vec<Text> {
        let max = max.clamp(1, i32::MAX as usize) as i32;
        let (mut parts, mut from) = (Vec::new(), (0, 0));
        while self.len - from.1 > max {
            let to = self.cut_point(from, max);
            parts.push(self.cut(from, to));
            from = to;
        }
        parts.push(self.cut(from, (self.text.len(), self.len)));
        parts
    }

    pub(crate) fn write_entities(&self) -> Vec<base::MessageEntity> {
        let mut spans: Vec<&Span> = self.spans.iter().filter(|s| s.end > s.start).collect();
        spans.sort_by_key(|s| (s.start, -s.end));
        spans.into_iter().map(Span::write).collect()
    }

    pub(crate) fn push(&mut self, c: char) {
        self.text.push(c);
        self.len += c.len_utf16() as i32;
    }

    pub(crate) fn push_str(&mut self, s: &str) {
        s.chars().for_each(|c| self.push(c));
    }

    pub(crate) fn span(&mut self, kind: Kind, start: i32) {
        self.spans.push(Span { kind, start, end: self.len });
    }

    pub(crate) fn parsed(self, mode: ParseMode) -> Text {
        match mode {
            ParseMode::Html | ParseMode::Markdown if self.unparsed => parser::parse(&self.text, mode),
            _ => self,
        }
    }

    fn wrap(mut self, kind: Kind, text: impl Into<Text>) -> Self {
        let start = self.len;
        self.append(text.into());
        self.span(kind, start);
        self
    }

    fn append(&mut self, other: Text) {
        if self.text.is_empty() && self.spans.is_empty() {
            *self = Text { unparsed: false, ..other };
            return;
        }
        self.unparsed = false;
        let shift = self.len;
        self.text.push_str(&other.text);
        self.len += other.len;
        self.spans.extend(other.spans.into_iter().map(|s| Span { start: s.start + shift, end: s.end + shift, ..s }));
    }

    fn boundary(&self, (mut byte, mut at): (usize, i32), unit: i32) -> (usize, i32) {
        for c in self.text[byte..].chars() {
            let next = at + c.len_utf16() as i32;
            if next > unit {
                break;
            }
            (byte, at) = (byte + c.len_utf8(), next);
        }
        (byte, at)
    }

    fn cut_point(&self, (byte, start): (usize, i32), max: i32) -> (usize, i32) {
        let (limit, half) = (start + max, start + max / 2);
        let (mut line, mut space, mut hard) = (None, None, None);
        let mut at = start;
        for (i, c) in self.text[byte..].char_indices() {
            let next = at + c.len_utf16() as i32;
            if next > limit && hard.is_some() {
                break;
            }
            let point = (byte + i + c.len_utf8(), next);
            match c {
                '\n' => line = Some(point),
                c if c.is_whitespace() => space = Some(point),
                _ => {}
            }
            (hard, at) = (Some(point), next);
        }
        let preferred = [line, space].into_iter().flatten().find(|p| p.1 > half);
        let mut cut = preferred.or(hard).unwrap_or((self.text.len(), self.len));
        let through = |s: &&Span, cut: i32| s.kind.is_atomic() && s.start < cut && cut < s.end;
        while let Some(s) = self.spans.iter().find(|s| through(s, cut.1)) {
            if s.start > start {
                cut = self.boundary((byte, start), s.start);
                continue;
            }
            // A span the part starts with stays whole when it fits; only a longer one is cut.
            if s.end - start <= max {
                cut = self.boundary((byte, start), s.end);
            }
            break;
        }
        cut
    }

    fn cut(&self, (from, start): (usize, i32), (to, end): (usize, i32)) -> Text {
        let clip = |s: &Span| {
            let (a, b) = (s.start.max(start), s.end.min(end));
            (a < b).then(|| Span { kind: s.kind.clone(), start: a - start, end: b - start })
        };
        let spans = self.spans.iter().filter_map(clip).collect();
        Text { text: self.text[from..to].into(), len: end - start, spans, unparsed: false }
    }
}

impl From<String> for Text {
    fn from(text: String) -> Self {
        let len = text.encode_utf16().count() as i32;
        Text { text, len, spans: Vec::new(), unparsed: true }
    }
}

impl From<&str> for Text {
    fn from(text: &str) -> Self {
        String::from(text).into()
    }
}

impl From<&String> for Text {
    fn from(text: &String) -> Self {
        text.as_str().into()
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Bound;

    use super::Text;
    use crate::enums::{MessageEntityType as T, ParseMode};
    use crate::raw::base::MessageEntity as E;

    fn show(t: &Text) -> (&str, Vec<String>) {
        let describe = |e: &E| match e {
            E::MessageEntityBold(e) => format!("bold {} {}", e.offset, e.length),
            E::MessageEntityItalic(e) => format!("italic {} {}", e.offset, e.length),
            E::MessageEntityUnderline(e) => format!("underline {} {}", e.offset, e.length),
            E::MessageEntityStrike(e) => format!("strike {} {}", e.offset, e.length),
            E::MessageEntitySpoiler(e) => format!("spoiler {} {}", e.offset, e.length),
            E::MessageEntityCode(e) => format!("code {} {}", e.offset, e.length),
            E::MessageEntityPre(e) => format!("pre:{} {} {}", e.language, e.offset, e.length),
            E::MessageEntityTextUrl(e) => format!("url:{} {} {}", e.url, e.offset, e.length),
            E::InputMessageEntityMentionName(e) => format!("mention {} {}", e.offset, e.length),
            E::MessageEntityBlockquote(e) => format!("quote:{} {} {}", e.collapsed, e.offset, e.length),
            E::MessageEntityCustomEmoji(e) => format!("emoji:{} {} {}", e.document_id, e.offset, e.length),
            E::MessageEntityFormattedDate(e) => format!("date:{}:{} {} {}", e.date, e.relative, e.offset, e.length),
            other => format!("{other:?}"),
        };
        (t.as_str(), t.write_entities().iter().map(describe).collect())
    }

    fn texts(parts: &[Text]) -> Vec<&str> {
        parts.iter().map(Text::as_str).collect()
    }

    #[test]
    fn offsets_count_utf16_units() {
        let t = Text::new().plain("😀 ").bold("é😀").plain(" ").custom_emoji("👍", 5368).link("x", "https://a.b");
        assert_eq!(
            show(&t),
            ("😀 é😀 👍x", vec!["bold 3 3".into(), "emoji:5368 7 2".into(), "url:https://a.b 9 1".into()])
        );
        assert_eq!((t.len(), t.is_empty(), Text::new().is_empty()), (10, false, true));
        let t = Text::new().plain("𝕏").plain(Text::from_html("<b>x</b>")).code(Text::from_markdown("**😀**"));
        assert_eq!(show(&t), ("𝕏x😀", vec!["bold 2 1".into(), "bold 3 2".into(), "code 3 2".into()]));
    }

    #[test]
    fn entities_nest() {
        let inner = Text::new().plain("a").italic("b😀").underline(Text::new().strike("c"));
        let t = Text::new().bold(inner).spoiler("d").blockquote(Text::new().pre("fn x()", "rust"), true);
        assert_eq!(
            show(&t),
            (
                "ab😀cdfn x()",
                vec![
                    "bold 0 5".into(),
                    "italic 1 3".into(),
                    "strike 4 1".into(),
                    "underline 4 1".into(),
                    "spoiler 5 1".into(),
                    "pre:rust 6 6".into(),
                    "quote:true 6 6".into()
                ]
            )
        );
        let t = Text::new().mention("me", 42).date_time("in 2h", 1_700_000_000, "r").bold("");
        assert_eq!(show(&t), ("mein 2h", vec!["mention 0 2".into(), "date:1700000000:true 2 5".into()]));
        let entities = Text::new().mention("me", 42).date_time("Mon", 1, "wDT").entities();
        let user = entities[0].user.as_ref().map(|u| u.id);
        assert_eq!((entities[0].kind, user, entities[1].kind), (T::TextMention, Some(42), T::DateTime));
        assert_eq!((entities[1].unix_time, entities[1].date_time_format.as_deref()), (Some(1), Some("wDT")));
    }

    #[test]
    fn nothing_needs_escaping() {
        let t = Text::new().bold("<b>&amp;</b> **x**").plain(String::from("[a](b)"));
        assert_eq!(show(&t), ("<b>&amp;</b> **x**[a](b)", vec!["bold 0 18".into()]));
        assert_eq!(show(&t.clone().parsed(ParseMode::Html)), show(&t));
        assert_eq!(show(&Text::from("<b>x</b>").parsed(ParseMode::Html)), ("x", vec!["bold 0 1".into()]));
        assert_eq!(show(&Text::from("<b>x</b>").parsed(ParseMode::Disabled)), ("<b>x</b>", vec![]));
        assert_eq!(show(&Text::new().plain("<b>x</b>").parsed(ParseMode::Html)), ("<b>x</b>", vec![]));
        assert_eq!(show(&Text::from("<b>x</b>").bold("").parsed(ParseMode::Html)), ("<b>x</b>", vec![]));
        assert_eq!(show(&Text::from("<b>x</b>").slice(..).parsed(ParseMode::Html)), ("<b>x</b>", vec![]));
        assert_eq!(show(&Text::from_html("1 &lt;b&gt; 2").parsed(ParseMode::Html)), ("1 <b> 2", vec![]));
        assert_eq!(show(&Text::from_markdown(r"\*\*x\*\*").parsed(ParseMode::Markdown)), ("**x**", vec![]));
    }

    #[test]
    fn slices_clip_entities() {
        let t = Text::from_html("😀<b>ab😀cd</b>ef");
        assert_eq!(show(&t), ("😀ab😀cdef", vec!["bold 2 6".into()]));
        assert_eq!(show(&t.slice(3..7)), ("b😀c", vec!["bold 0 4".into()]));
        assert_eq!(show(&t.slice(5..)), ("😀cdef", vec!["bold 0 4".into()]));
        assert_eq!(show(&t.slice(..=2)), ("😀a", vec!["bold 2 1".into()]));
        assert_eq!(show(&t.slice(8..)), ("ef", vec![]));
        assert_eq!(show(&t.slice(..1)), ("", vec![]));
        assert_eq!(show(&t.slice((Bound::Included(7), Bound::Excluded(3)))), ("", vec![]));
        assert_eq!(show(&t.slice(..99)), show(&t));
    }

    #[test]
    fn splits_at_lines_then_spaces() {
        assert_eq!(texts(&Text::from("aaaa bbbb\ncccc dddd").split(10)), ["aaaa bbbb\n", "cccc dddd"]);
        assert_eq!(texts(&Text::from("aaaa bbbb cccc").split(10)), ["aaaa bbbb ", "cccc"]);
        assert_eq!(texts(&Text::from("a\nbbbbbbbbbbbb").split(8)), ["a\nbbbbbb", "bbbbbb"]);
        assert_eq!(texts(&Text::from("😀😀😀").split(3)), ["😀", "😀", "😀"]);
        assert_eq!(texts(&Text::from("😀x").split(0)), ["😀", "x"]);
        assert_eq!(texts(&Text::new().split(4096)), [""]);
        let long = Text::from("word ".repeat(2000)).split(4096);
        assert_eq!(long.iter().map(Text::len).collect::<Vec<_>>(), [4095, 4095, 1810]);
    }

    #[test]
    fn splits_keep_entities_whole() {
        let parts = Text::new().bold("aaaa bbbb cccc").split(10);
        assert_eq!(
            parts.iter().map(show).collect::<Vec<_>>(),
            [("aaaa bbbb ", vec!["bold 0 10".into()]), ("cccc", vec!["bold 0 4".into()])]
        );
        let parts = Text::new().plain("ab").date_time("tomorrow", 1, "").split(8);
        assert_eq!(
            parts.iter().map(show).collect::<Vec<_>>(),
            [("ab", vec![]), ("tomorrow", vec!["date:1:false 0 8".into()])]
        );
        let parts = Text::new().date_time("abcdefg hij", 1, "").plain("yyyy").split(12);
        assert_eq!(
            parts.iter().map(show).collect::<Vec<_>>(),
            [("abcdefg hij", vec!["date:1:false 0 11".into()]), ("yyyy", vec![])]
        );
        let parts = Text::new().plain("aaaaaa").custom_emoji("👍", 7).plain("b").split(7);
        assert_eq!(
            parts.iter().map(show).collect::<Vec<_>>(),
            [("aaaaaa", vec![]), ("👍b", vec!["emoji:7 0 2".into()])]
        );
    }

    #[test]
    fn telegram_units_are_utf16() {
        let t = Text::new().plain("😀").bold("b");
        assert_eq!((t.as_str().len(), t.as_str().chars().count(), t.len()), (5, 2, 3));
        assert_eq!(show(&t).1, ["bold 2 1"]);

        let t = Text::new().plain("a".repeat(4090)).bold("aaaaa😀xy").italic("z");
        assert_eq!(t.len(), 4100);
        let parts = t.split(4096);
        assert_eq!(texts(&parts), ["a".repeat(4095).as_str(), "😀xyz"]);
        assert_eq!((parts[0].len(), parts[1].len()), (4095, 5));
        assert_eq!(show(&parts[0]).1, ["bold 4090 5"]);
        assert_eq!(show(&parts[1]).1, ["bold 0 4", "italic 4 1"]);

        let t = Text::new().plain("x").bold("😀😀").plain("y");
        assert_eq!(show(&t.slice(1..3)), ("😀", vec!["bold 0 2".into()]));
        assert_eq!(show(&t.slice(2..4)), ("😀", vec!["bold 0 2".into()]));
        assert_eq!(show(&t.slice(2..5)), ("😀😀", vec!["bold 0 4".into()]));
        assert_eq!(show(&t.slice(4..)), ("😀y", vec!["bold 0 2".into()]));
    }

    #[test]
    fn split_parts_join_back() {
        let t = Text::new()
            .plain("😀 one\n")
            .bold(Text::new().italic("two 𝕏𝕏 three").plain(" four"))
            .custom_emoji("👍", 1)
            .plain("\nfive six😀")
            .link("seven eight", "https://x.y");
        for max in 2..=t.len() + 1 {
            let parts = t.split(max);
            assert!(parts.iter().all(|p| p.len() <= max), "a part is longer than {max}");
            assert_eq!(parts.iter().map(Text::as_str).collect::<String>(), t.as_str());
            assert_eq!(parts.iter().map(Text::len).sum::<usize>(), t.len());
        }
    }

    #[test]
    fn unparsed_text_is_plain() {
        let text = Text::from("<b>*x*</b>");
        assert_eq!(text.to_html(), "&lt;b&gt;*x*&lt;/b&gt;");
        assert_eq!(text.to_markdown(), r"<b>\*x\*</b>");
    }
}
