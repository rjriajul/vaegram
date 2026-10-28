mod html;
mod markdown;

use crate::enums::ParseMode;
use crate::raw::{base, types};
use crate::types::Text;

// Formatting nested deeper than this is dropped: each out-of-order close reopens every span inside it, so an
// unbounded stack would make the span count quadratic in the input.
pub(crate) const MAX_NESTING: usize = 64;

pub fn parse(text: &str, mode: ParseMode) -> Text {
    match mode {
        ParseMode::Disabled => Text::new().plain(text),
        ParseMode::Html => html::parse(text),
        ParseMode::Markdown => markdown::parse(text),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Kind {
    Bold,
    Italic,
    Underline,
    Strike,
    Spoiler,
    Code,
    Pre(String),
    Link(String),
    Mention(i64),
    Blockquote(bool),
    CustomEmoji(i64),
    DateTime(i32, u8),
}

impl Kind {
    fn link(url: String) -> Kind {
        match url.strip_prefix("tg://user?id=").and_then(|id| id.parse().ok()) {
            Some(user_id) => Kind::Mention(user_id),
            None => Kind::Link(url),
        }
    }

    pub(crate) fn date_time(unix_time: i32, format: &str) -> Kind {
        let flag = |c| match c {
            'r' => 1,
            't' => 2,
            'T' => 4,
            'd' => 8,
            'D' => 16,
            'w' => 32,
            _ => 0,
        };
        Kind::DateTime(unix_time, format.chars().fold(0, |flags, c| flags | flag(c)))
    }

    pub(crate) fn is_atomic(&self) -> bool {
        matches!(self, Kind::CustomEmoji(_) | Kind::DateTime(..))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) kind: Kind,
    pub(crate) start: i32,
    pub(crate) end: i32,
}

impl Span {
    pub(crate) fn write(&self) -> base::MessageEntity {
        use base::MessageEntity as E;
        let (offset, length) = (self.start, self.end - self.start);
        match &self.kind {
            Kind::Bold => E::MessageEntityBold(Box::new(types::MessageEntityBold { offset, length })),
            Kind::Italic => E::MessageEntityItalic(Box::new(types::MessageEntityItalic { offset, length })),
            Kind::Underline => E::MessageEntityUnderline(Box::new(types::MessageEntityUnderline { offset, length })),
            Kind::Strike => E::MessageEntityStrike(Box::new(types::MessageEntityStrike { offset, length })),
            Kind::Spoiler => E::MessageEntitySpoiler(Box::new(types::MessageEntitySpoiler { offset, length })),
            Kind::Code => E::MessageEntityCode(Box::new(types::MessageEntityCode { offset, length })),
            Kind::Pre(language) => {
                E::MessageEntityPre(Box::new(types::MessageEntityPre { offset, length, language: language.clone() }))
            }
            Kind::Link(url) => {
                E::MessageEntityTextUrl(Box::new(types::MessageEntityTextUrl { offset, length, url: url.clone() }))
            }
            &Kind::Mention(user_id) => {
                let user_id = base::InputUser::InputUser(Box::new(types::InputUser { user_id, access_hash: 0 }));
                let mention = types::InputMessageEntityMentionName { offset, length, user_id };
                E::InputMessageEntityMentionName(Box::new(mention))
            }
            &Kind::Blockquote(collapsed) => {
                E::MessageEntityBlockquote(Box::new(types::MessageEntityBlockquote { collapsed, offset, length }))
            }
            &Kind::CustomEmoji(document_id) => {
                E::MessageEntityCustomEmoji(Box::new(types::MessageEntityCustomEmoji { offset, length, document_id }))
            }
            &Kind::DateTime(date, flags) => {
                let has = |bit: u8| flags & (1 << bit) != 0;
                E::MessageEntityFormattedDate(Box::new(types::MessageEntityFormattedDate {
                    relative: has(0),
                    short_time: has(1),
                    long_time: has(2),
                    short_date: has(3),
                    long_date: has(4),
                    day_of_week: has(5),
                    offset,
                    length,
                    date,
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::enums::ParseMode;
    use crate::raw::base::MessageEntity as E;

    fn run(text: &str, mode: ParseMode) -> (String, Vec<String>) {
        let text = parse(text, mode);
        let entities = text
            .write_entities()
            .iter()
            .map(|e| match e {
                E::MessageEntityBold(e) => format!("bold {} {}", e.offset, e.length),
                E::MessageEntityItalic(e) => format!("italic {} {}", e.offset, e.length),
                E::MessageEntityUnderline(e) => format!("underline {} {}", e.offset, e.length),
                E::MessageEntityStrike(e) => format!("strike {} {}", e.offset, e.length),
                E::MessageEntitySpoiler(e) => format!("spoiler {} {}", e.offset, e.length),
                E::MessageEntityCode(e) => format!("code {} {}", e.offset, e.length),
                E::MessageEntityPre(e) => format!("pre:{} {} {}", e.language, e.offset, e.length),
                E::MessageEntityTextUrl(e) => format!("url:{} {} {}", e.url, e.offset, e.length),
                E::MessageEntityBlockquote(e) => format!("quote:{} {} {}", e.collapsed, e.offset, e.length),
                E::MessageEntityCustomEmoji(e) => format!("emoji:{} {} {}", e.document_id, e.offset, e.length),
                E::InputMessageEntityMentionName(e) => format!("mention {} {}", e.offset, e.length),
                E::MessageEntityFormattedDate(e) => {
                    let on = [e.relative, e.short_time, e.long_time, e.short_date, e.long_date, e.day_of_week];
                    let flags: String = "rtTdDw".chars().zip(on).filter_map(|(c, on)| on.then_some(c)).collect();
                    format!("date:{}:{flags} {} {}", e.date, e.offset, e.length)
                }
                other => format!("{other:?}"),
            })
            .collect();
        (text.as_str().into(), entities)
    }

    fn html(text: &str) -> (String, Vec<String>) {
        run(text, ParseMode::Html)
    }

    fn md(text: &str) -> (String, Vec<String>) {
        run(text, ParseMode::Markdown)
    }

    #[test]
    fn plain_text_is_untouched() {
        assert_eq!(run("<b>*hi*</b>", ParseMode::Disabled), ("<b>*hi*</b>".into(), vec![]));
    }

    #[test]
    fn html_tags_become_entities() {
        assert_eq!(html("<b>bold</b> <i>it</i>"), ("bold it".into(), vec!["bold 0 4".into(), "italic 5 2".into()]));
        assert_eq!(html("<b>a<i>b</i></b>"), ("ab".into(), vec!["bold 0 2".into(), "italic 1 1".into()]));
        assert_eq!(
            html("<u>u</u><s>s</s><del>d</del><tg-spoiler>x</tg-spoiler><span class=\"tg-spoiler\">y</span>"),
            (
                "usdxy".into(),
                vec![
                    "underline 0 1".into(),
                    "strike 1 1".into(),
                    "strike 2 1".into(),
                    "spoiler 3 1".into(),
                    "spoiler 4 1".into()
                ]
            )
        );
        assert_eq!(html("<code>x</code>"), ("x".into(), vec!["code 0 1".into()]));
        assert_eq!(
            html("<pre><code class=\"language-rust\">fn x()</code></pre>"),
            ("fn x()".into(), vec!["pre:rust 0 6".into()])
        );
        assert_eq!(html("<pre>p</pre>"), ("p".into(), vec!["pre: 0 1".into()]));
        assert_eq!(
            html("<a href=\"https://a.b/?x=1&amp;y=2\">link</a>"),
            ("link".into(), vec!["url:https://a.b/?x=1&y=2 0 4".into()])
        );
        assert_eq!(html("<a href=https://x.y/>u</a><br/>"), ("u".into(), vec!["url:https://x.y/ 0 1".into()]));
        assert_eq!(html("<a href='tg://user?id=42'>me</a>"), ("me".into(), vec!["mention 0 2".into()]));
        assert_eq!(html("<blockquote expandable>q</blockquote>"), ("q".into(), vec!["quote:true 0 1".into()]));
        assert_eq!(html("<tg-emoji emoji-id=\"5368\">👍</tg-emoji>"), ("👍".into(), vec!["emoji:5368 0 2".into()]));
    }

    #[test]
    fn html_tg_time_becomes_a_formatted_date() {
        assert_eq!(
            html("<tg-time unix=\"1647531900\" format=\"wDT\">22:45 tomorrow</tg-time>"),
            ("22:45 tomorrow".into(), vec!["date:1647531900:TDw 0 14".into()])
        );
        assert_eq!(html("😀<tg-time unix=1 format=r>now</tg-time>"), ("😀now".into(), vec!["date:1:r 2 3".into()]));
        assert_eq!(html("<tg-time unix=5>t</tg-time>"), ("t".into(), vec!["date:5: 0 1".into()]));
        assert_eq!(html("<tg-time format=t>no unix</tg-time>"), ("no unix".into(), vec![]));
        assert_eq!(html("<tg-time unix=soon>bad</tg-time>"), ("bad".into(), vec![]));
    }

    #[test]
    fn html_text_is_decoded_and_kept() {
        assert_eq!(html("1 &lt; 2 &amp;&amp; 3 > 2 &#33;&#x21; &bogus;"), ("1 < 2 && 3 > 2 !! &bogus;".into(), vec![]));
        assert_eq!(html("a < b"), ("a < b".into(), vec![]));
        assert_eq!(html("<unknown>x</unknown>"), ("x".into(), vec![]));
        assert_eq!(html("<b>unclosed"), ("unclosed".into(), vec![]));
        assert_eq!(html("</b>x"), ("x".into(), vec![]));
        assert_eq!(html("<a>no href</a>"), ("no href".into(), vec![]));
    }

    #[test]
    fn offsets_count_utf16_units() {
        assert_eq!(html("😀<b>é😀</b>"), ("😀é😀".into(), vec!["bold 2 3".into()]));
        assert_eq!(md("😀 **😀**"), ("😀 😀".into(), vec!["bold 3 2".into()]));
    }

    #[test]
    fn crossing_tags_close_and_reopen_so_entities_nest() {
        let nested = vec!["bold 0 5".into(), "italic 3 2".into(), "italic 5 3".into()];
        assert_eq!(md("**ab __cd** ef__"), ("ab cd ef".into(), nested.clone()));
        assert_eq!(html("<b>ab <i>cd</b> ef</i>"), ("ab cd ef".into(), nested));
        assert_eq!(html("<b><i>x</b></i>y"), ("xy".into(), vec!["italic 0 1".into(), "bold 0 1".into()]));
    }

    #[test]
    fn markdown_delimiters_become_entities() {
        assert_eq!(
            md("**b** __i__ --u-- ~~s~~ ||p||"),
            (
                "b i u s p".into(),
                vec![
                    "bold 0 1".into(),
                    "italic 2 1".into(),
                    "underline 4 1".into(),
                    "strike 6 1".into(),
                    "spoiler 8 1".into()
                ]
            )
        );
        assert_eq!(md("**a __b__**"), ("a b".into(), vec!["bold 0 3".into(), "italic 2 1".into()]));
        assert_eq!(md("`**x**`"), ("**x**".into(), vec!["code 0 5".into()]));
        assert_eq!(md("```rust\nfn x()\n```"), ("fn x()".into(), vec!["pre:rust 0 6".into()]));
        assert_eq!(md("```plain```"), ("plain".into(), vec!["pre: 0 5".into()]));
        assert_eq!(
            md("[**vae**](https://x.y) end"),
            ("vae end".into(), vec!["bold 0 3".into(), "url:https://x.y 0 3".into()])
        );
        assert_eq!(md("[me](tg://user?id=7)"), ("me".into(), vec!["mention 0 2".into()]));
    }

    #[test]
    fn markdown_keeps_what_is_not_formatting() {
        assert_eq!(md("2 ** 3 and a -- b"), ("2 ** 3 and a -- b".into(), vec![]));
        assert_eq!(md(r"\*\*not bold\*\* \[x\]"), ("**not bold** [x]".into(), vec![]));
        assert_eq!(md("[not a link] (x)"), ("[not a link] (x)".into(), vec![]));
        assert_eq!(md("`unclosed"), ("`unclosed".into(), vec![]));
    }

    #[test]
    fn markdown_quotes() {
        assert_eq!(md("> one\n> two\nafter"), ("one\ntwo\nafter".into(), vec!["quote:false 0 7".into()]));
        assert_eq!(md("x\n>last"), ("x\nlast".into(), vec!["quote:false 2 4".into()]));
    }

    #[test]
    fn out_of_order_html_closes_stay_linear() {
        // Each </i> closes an <i> under every <b>, which closes and reopens them all.
        let n = 200;
        let html = format!("{}{}{}", "<i>".repeat(n), "<b>".repeat(n), "x</i>".repeat(n));
        let text = parse(&html, ParseMode::Html);
        assert_eq!(text.text, "x".repeat(n));
        assert!(text.spans.len() <= super::MAX_NESTING * n, "{} spans", text.spans.len());
    }

    #[test]
    fn tags_past_the_cap_leave_the_outer_ones_alone() {
        // The inner <i> is the 65th open tag: its </i> must not end the outer one.
        let html = format!("<i>{}<i>x</i>y</i>", "<b>".repeat(63));
        let (text, entities) = run(&html, ParseMode::Html);
        let italic: Vec<&str> = entities.iter().map(String::as_str).filter(|e| e.starts_with("italic")).collect();
        assert_eq!((text.as_str(), italic), ("xy", vec!["italic 0 2"]));
        let unknown = format!("{}<b>a</b>", "<x>".repeat(70));
        assert_eq!(run(&unknown, ParseMode::Html), ("a".into(), vec!["bold 0 1".into()]));
    }
}
