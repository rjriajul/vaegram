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

pub(crate) fn unparse(text: &Text, mode: ParseMode) -> String {
    match mode {
        ParseMode::Disabled => text.text.clone(),
        ParseMode::Html => html::unparse(text),
        ParseMode::Markdown => markdown::unparse(text),
    }
}

trait Markup {
    fn open(&mut self, kind: &Kind);
    fn close(&mut self, kind: &Kind);
    fn char(&mut self, at: i32, c: char);
}

fn walk(text: &str, mut spans: Vec<Span>, out: &mut impl Markup) {
    spans.retain(|s| s.start >= 0 && s.end > s.start);
    spans.sort_by_key(|s| (s.start, -s.end));
    let mut next = spans.iter().peekable();
    let mut open: Vec<&Span> = Vec::new();
    let mut at = 0;
    for c in text.chars().map(Some).chain([None]) {
        if let Some(k) = open.iter().position(|s| s.end <= at) {
            let closed: Vec<&Span> = open.drain(k..).rev().inspect(|s| out.close(&s.kind)).collect();
            for s in closed.into_iter().rev().filter(|s| s.end > at) {
                out.open(&s.kind);
                open.push(s);
            }
        }
        while let Some(s) = next.next_if(|s| s.start <= at) {
            if s.end > at {
                out.open(&s.kind);
                open.push(s);
            }
        }
        let Some(c) = c else { break };
        out.char(at, c);
        at += c.len_utf16() as i32;
    }
    for s in open.iter().rev() {
        out.close(&s.kind);
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
    pub(crate) fn read(entity: &base::MessageEntity) -> Option<Span> {
        use base::MessageEntity as E;
        let (kind, offset, length) = match entity {
            E::MessageEntityBold(e) => (Kind::Bold, e.offset, e.length),
            E::MessageEntityItalic(e) => (Kind::Italic, e.offset, e.length),
            E::MessageEntityUnderline(e) => (Kind::Underline, e.offset, e.length),
            E::MessageEntityStrike(e) => (Kind::Strike, e.offset, e.length),
            E::MessageEntitySpoiler(e) => (Kind::Spoiler, e.offset, e.length),
            E::MessageEntityCode(e) => (Kind::Code, e.offset, e.length),
            E::MessageEntityPre(e) => (Kind::Pre(e.language.clone()), e.offset, e.length),
            E::MessageEntityTextUrl(e) => (Kind::Link(e.url.clone()), e.offset, e.length),
            E::MessageEntityMentionName(e) => (Kind::Mention(e.user_id), e.offset, e.length),
            E::InputMessageEntityMentionName(e) => match &e.user_id {
                base::InputUser::InputUser(user) => (Kind::Mention(user.user_id), e.offset, e.length),
                _ => return None,
            },
            E::MessageEntityBlockquote(e) => (Kind::Blockquote(e.collapsed), e.offset, e.length),
            E::MessageEntityCustomEmoji(e) => (Kind::CustomEmoji(e.document_id), e.offset, e.length),
            E::MessageEntityFormattedDate(e) => {
                let on = [e.relative, e.short_time, e.long_time, e.short_date, e.long_date, e.day_of_week];
                let flags = on.iter().enumerate().fold(0, |flags, (bit, &on)| flags | (u8::from(on) << bit));
                (Kind::DateTime(e.date, flags), e.offset, e.length)
            }
            _ => return None,
        };
        Some(Span { kind, start: offset, end: offset.saturating_add(length) })
    }

    pub(crate) fn write(&self) -> base::MessageEntity {
        use base::MessageEntity as E;
        let (offset, length) = (self.start, self.end - self.start);
        match &self.kind {
            Kind::Bold => E::MessageEntityBold(types::MessageEntityBold { offset, length }),
            Kind::Italic => E::MessageEntityItalic(types::MessageEntityItalic { offset, length }),
            Kind::Underline => E::MessageEntityUnderline(types::MessageEntityUnderline { offset, length }),
            Kind::Strike => E::MessageEntityStrike(types::MessageEntityStrike { offset, length }),
            Kind::Spoiler => E::MessageEntitySpoiler(types::MessageEntitySpoiler { offset, length }),
            Kind::Code => E::MessageEntityCode(types::MessageEntityCode { offset, length }),
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
    use super::{Kind, Span, parse, unparse};
    use crate::enums::ParseMode;
    use crate::raw::base::MessageEntity as E;
    use crate::raw::types;
    use crate::types::Text;

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

    #[test]
    fn html_tags_nest_and_overlap() {
        assert_eq!(html("<b>a<b>b</b>c</b>"), ("abc".into(), vec!["bold 0 3".into(), "bold 1 1".into()]));
        assert_eq!(
            html("<b><i>x</b>y</i>"),
            ("xy".into(), vec!["italic 0 1".into(), "bold 0 1".into(), "italic 1 1".into()])
        );
        assert_eq!(html("<B>x</B><EM>y</em>"), ("xy".into(), vec!["bold 0 1".into(), "italic 1 1".into()]));
        assert_eq!(html("<b></b>x<i></i>"), ("x".into(), vec![]));
        assert_eq!(html("<code>a</code><pre>b</pre>"), ("ab".into(), vec!["code 0 1".into(), "pre: 1 1".into()]));
    }

    #[test]
    fn html_malformed_input_stays_text() {
        assert_eq!(html("a<1 and <b x"), ("a<1 and <b x".into(), vec![]));
        assert_eq!(html("x &amp y &#xD800; &#; &#99999999;"), ("x &amp y &#xD800; &#; &#99999999;".into(), vec![]));
        assert_eq!(html("<tg-emoji emoji-id=\"nope\">e</tg-emoji>"), ("e".into(), vec![]));
        assert_eq!(html("<a href=\"x>y\">l</a>"), ("l".into(), vec!["url:x>y 0 1".into()]));
        assert_eq!(html("<a href=\"tg://user?id=abc\">l</a>"), ("l".into(), vec!["url:tg://user?id=abc 0 1".into()]));
        assert_eq!(html("<span>s</span>"), ("s".into(), vec![]));
    }

    #[test]
    fn decoded_entities_count_utf16_units() {
        assert_eq!(html("&#128512;<b>x</b>"), ("😀x".into(), vec!["bold 2 1".into()]));
        assert_eq!(html("&#x1F600;&nbsp;<i>y</i>"), ("😀\u{a0}y".into(), vec!["italic 3 1".into()]));
        assert_eq!(md("😀[x](u) `😀`"), ("😀x 😀".into(), vec!["url:u 2 1".into(), "code 4 2".into()]));
        assert_eq!(md("> 😀\nz"), ("😀\nz".into(), vec!["quote:false 0 2".into()]));
    }

    #[test]
    fn markdown_edge_cases_parse_or_stay_text() {
        let mut same = md("**__x__**");
        same.1.sort();
        assert_eq!(same, ("x".into(), vec!["bold 0 1".into(), "italic 0 1".into()]));
        assert_eq!(md("**a** **b**"), ("a b".into(), vec!["bold 0 1".into(), "bold 2 1".into()]));
        assert_eq!(md("**a"), ("**a".into(), vec![]));
        assert_eq!(md(r"\\ \a"), (r"\ \a".into(), vec![]));
        assert_eq!(md("``"), ("".into(), vec![]));
        assert_eq!(md("```\ncode\n```"), ("code".into(), vec!["pre: 0 4".into()]));
        assert_eq!(md("```two words\nx```"), ("two words\nx".into(), vec!["pre: 0 11".into()]));
        assert_eq!(md("[a](b"), ("[a](b".into(), vec![]));
    }

    #[test]
    fn entities_fit_the_text_and_are_ordered() {
        let inputs = [
            "<b>a<i>b<u>c</b>d</i>e",
            "</i></b><b>😀",
            "<pre><code class=\"language-x\">a</pre></code>",
            "**a __b** c__ ||d",
            "> q\n> **😀**\n`x` [y](z) ~~",
            "<a href=\"u\"><b>😀</b></a>&#65;",
        ];
        for input in inputs {
            for mode in [ParseMode::Html, ParseMode::Markdown] {
                let parsed = parse(input, mode);
                let (text, entities) = (parsed.text.clone(), parsed.write_entities());
                let len = text.encode_utf16().count() as i32;
                let mut last = (0, i32::MAX);
                for e in &entities {
                    let (offset, length) = bounds(e);
                    assert!(offset >= 0 && length > 0 && offset + length <= len, "{input:?} {mode:?} {e:?}");
                    assert!((offset, -length) >= (last.0, -last.1), "{input:?} {mode:?} out of order");
                    last = (offset, length);
                }
            }
        }
    }

    fn round_trip(mode: ParseMode, inputs: &[&str]) {
        let normal = |t: &Text| {
            let mut entities: Vec<String> = t.write_entities().iter().map(|e| format!("{e:?}")).collect();
            entities.sort();
            (t.text.clone(), entities)
        };
        for input in inputs {
            let text = parse(input, mode);
            assert!(!text.spans.is_empty(), "{input:?} has no formatting to keep");
            let output = unparse(&text, mode);
            assert_eq!(normal(&parse(&output, mode)), normal(&text), "{input:?} became {output:?}");
        }
    }

    #[test]
    fn html_output_round_trips() {
        round_trip(
            ParseMode::Html,
            &[
                "<b>a<i>b</i></b> c",
                "<b>ab <i>cd</b> ef</i>",
                "😀<b>é😀</b>😀",
                "<u>u</u><s>s</s><tg-spoiler>p</tg-spoiler><code>c</code><pre>p</pre>",
                "<pre><code class=\"language-rust\">fn x() -> u8 {}</code></pre>",
                "<a href=\"tg://user?id=42\">me</a>",
                "<tg-emoji emoji-id=\"5368\">👍</tg-emoji>",
                "<tg-time unix=\"1647531900\" format=\"wDT\">t</tg-time> <tg-time unix=5>u</tg-time>",
                "<blockquote expandable>q\nr</blockquote><blockquote>s</blockquote>",
                "1 &lt; 2 &amp;&amp; \"q\" &gt; <a href=\"https://a.b/?x=1&amp;y=&quot;2&quot;\">&amp;l</a>",
            ],
        );
    }

    #[test]
    fn markdown_output_round_trips() {
        round_trip(
            ParseMode::Markdown,
            &[
                "**b** __i__ --u-- ~~s~~ ||p||",
                "**ab __cd** ef__",
                "😀 **😀** [😀](u)",
                "```rust\nfn x() {}\n```",
                "`co*de` and ``` ```",
                "[**vae**](https://x.y) end",
                "[me](tg://user?id=7)",
                "> one\n> two\nafter",
                "x\n> last",
                "> q\n> **😀**\n`x` [y](z)",
                r"**\*\*not bold\*\* \[x\] a\\b \> c**",
                "**a**\n> \\> b\n\n**c**",
                "> a\n> `b`",
                "> `a`\nb",
            ],
        );
    }

    #[test]
    fn html_output_is_exact() {
        let text = Text::new().bold("b").plain(" ").link("l", "https://t.me/");
        assert_eq!(text.to_html(), "<b>b</b> <a href=\"https://t.me/\">l</a>");
        assert_eq!(text.to_markdown(), "**b** [l](https://t.me/)");
        let text = Text::new().pre("<&>", "x\"y").plain(" ").mention("m", 4).date_time("d", 9, "rd");
        assert_eq!(
            text.to_html(),
            "<pre><code class=\"language-x&quot;y\">&lt;&amp;&gt;</code></pre> <a href=\"tg://user?id=4\">m</a>\
             <tg-time unix=\"9\" format=\"rd\">d</tg-time>"
        );
    }

    #[test]
    fn markdown_output_escapes() {
        let text = Text::new().plain("a*b_c-d [x] ~|`\\\n> e");
        let output = text.to_markdown();
        assert_eq!(output, "a\\*b\\_c\\-d \\[x\\] \\~\\|\\`\\\\\n\\> e");
        assert_eq!(Text::from_markdown(&output).as_str(), "a*b_c-d [x] ~|`\\\n> e");
        assert_eq!(Text::new().link("l", "https://x.y/(a)").to_markdown(), "[l](https://x.y/(a%29)");
    }

    #[test]
    fn markdown_drops_what_it_cannot_express() {
        let cases = [
            (Text::new().custom_emoji("👍", 5), "👍"),
            (Text::new().date_time("t", 1, "r"), "t"),
            (Text::new().code("a`b"), "a\\`b"),
            (Text::new().pre("a```b", ""), "a\\`\\`\\`b"),
            (Text::new().link("a]b", "u"), "a\\]b"),
            (Text::new().plain("x ").blockquote("q", false), "x q"),
            (Text::new().blockquote("q\n", false), "q\n"),
            (Text::new().blockquote("q", true), "> q"),
            (Text::new().bold(Text::new().plain("a").bold("b")), "**ab**"),
            (Text::new().code(Text::new().plain("a").bold("x")), "`ax`"),
            (Text::new().pre("a\n", "").blockquote("q", false), "```\na\n\n```q"),
            (Text::new().blockquote("a", false).plain("\n").blockquote("b", false), "> a\nb"),
        ];
        for (text, markdown) in cases {
            assert_eq!(text.to_markdown(), markdown, "{text:?}");
            assert_eq!(Text::from_markdown(markdown).as_str(), text.as_str(), "{markdown:?} lost text");
        }
    }

    #[test]
    fn received_entities_become_spans() {
        let mention = types::MessageEntityMentionName { offset: 0, length: 2, user_id: 7 };
        let span = |kind, start, end| Some(Span { kind, start, end });
        assert_eq!(Span::read(&E::MessageEntityMentionName(Box::new(mention))), span(Kind::Mention(7), 0, 2));
        let date = types::MessageEntityFormattedDate {
            relative: true,
            short_time: false,
            long_time: false,
            short_date: false,
            long_date: true,
            day_of_week: false,
            offset: 1,
            length: 3,
            date: 9,
        };
        assert_eq!(Span::read(&E::MessageEntityFormattedDate(Box::new(date))), span(Kind::DateTime(9, 0b10001), 1, 4));
        let link = types::MessageEntityTextUrl { offset: 0, length: 1, url: "u".into() };
        assert_eq!(Span::read(&E::MessageEntityTextUrl(Box::new(link))), span(Kind::Link("u".into()), 0, 1));
        // Telegram finds these again in the sent text, so printing them as markup would add nothing.
        let detected = [
            E::MessageEntityUrl(types::MessageEntityUrl { offset: 0, length: 1 }),
            E::MessageEntityHashtag(types::MessageEntityHashtag { offset: 0, length: 1 }),
            E::MessageEntityDiffInsert(types::MessageEntityDiffInsert { offset: 0, length: 1 }),
        ];
        assert!(detected.iter().all(|e| Span::read(e).is_none()));

        let all = "<b>b</b><i>i</i><u>u</u><s>s</s><tg-spoiler>p</tg-spoiler><code>c</code><pre>p</pre>\
                   <a href=\"x\">l</a><a href=\"tg://user?id=4\">m</a><blockquote expandable>q</blockquote>\
                   <tg-emoji emoji-id=\"5\">e</tg-emoji><tg-time unix=\"1\" format=\"rtTdDw\">t</tg-time>";
        for s in parse(all, ParseMode::Html).spans {
            assert_eq!(Span::read(&s.write()).as_ref(), Some(&s));
        }
    }

    fn bounds(e: &E) -> (i32, i32) {
        match e {
            E::MessageEntityBold(e) => (e.offset, e.length),
            E::MessageEntityItalic(e) => (e.offset, e.length),
            E::MessageEntityUnderline(e) => (e.offset, e.length),
            E::MessageEntityStrike(e) => (e.offset, e.length),
            E::MessageEntitySpoiler(e) => (e.offset, e.length),
            E::MessageEntityCode(e) => (e.offset, e.length),
            E::MessageEntityPre(e) => (e.offset, e.length),
            E::MessageEntityTextUrl(e) => (e.offset, e.length),
            E::MessageEntityBlockquote(e) => (e.offset, e.length),
            other => panic!("unexpected entity {other:?}"),
        }
    }
}
