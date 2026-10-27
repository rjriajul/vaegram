mod html;
mod markdown;

use crate::enums::ParseMode;
use crate::raw::{base, types};

pub fn parse(text: &str, mode: ParseMode) -> (String, Vec<base::MessageEntity>) {
    let out = match mode {
        ParseMode::Disabled => return (text.to_owned(), Vec::new()),
        ParseMode::Html => html::parse(text),
        ParseMode::Markdown => markdown::parse(text),
    };
    let mut spans = out.spans;
    spans.retain(|s| s.end > s.start);
    spans.sort_by_key(|s| (s.start, -s.end));
    (out.text, spans.into_iter().map(Span::into_entity).collect())
}

enum Kind {
    Bold,
    Italic,
    Underline,
    Strike,
    Spoiler,
    Code,
    Pre(String),
    Link(String),
    Blockquote(bool),
    CustomEmoji(i64),
}

struct Span {
    kind: Kind,
    start: i32,
    end: i32,
}

impl Span {
    fn into_entity(self) -> base::MessageEntity {
        use base::MessageEntity as E;
        let (offset, length) = (self.start, self.end - self.start);
        match self.kind {
            Kind::Bold => E::MessageEntityBold(Box::new(types::MessageEntityBold { offset, length })),
            Kind::Italic => E::MessageEntityItalic(Box::new(types::MessageEntityItalic { offset, length })),
            Kind::Underline => E::MessageEntityUnderline(Box::new(types::MessageEntityUnderline { offset, length })),
            Kind::Strike => E::MessageEntityStrike(Box::new(types::MessageEntityStrike { offset, length })),
            Kind::Spoiler => E::MessageEntitySpoiler(Box::new(types::MessageEntitySpoiler { offset, length })),
            Kind::Code => E::MessageEntityCode(Box::new(types::MessageEntityCode { offset, length })),
            Kind::Pre(language) => E::MessageEntityPre(Box::new(types::MessageEntityPre { offset, length, language })),
            Kind::Link(url) => match url.strip_prefix("tg://user?id=").and_then(|id| id.parse().ok()) {
                Some(user_id) => {
                    let user_id = base::InputUser::InputUser(Box::new(types::InputUser { user_id, access_hash: 0 }));
                    let mention = types::InputMessageEntityMentionName { offset, length, user_id };
                    E::InputMessageEntityMentionName(Box::new(mention))
                }
                None => E::MessageEntityTextUrl(Box::new(types::MessageEntityTextUrl { offset, length, url })),
            },
            Kind::Blockquote(collapsed) => {
                E::MessageEntityBlockquote(Box::new(types::MessageEntityBlockquote { collapsed, offset, length }))
            }
            Kind::CustomEmoji(document_id) => {
                E::MessageEntityCustomEmoji(Box::new(types::MessageEntityCustomEmoji { offset, length, document_id }))
            }
        }
    }
}

#[derive(Default)]
struct Out {
    text: String,
    len: i32,
    spans: Vec<Span>,
}

impl Out {
    fn push(&mut self, c: char) {
        self.text.push(c);
        self.len += c.len_utf16() as i32;
    }

    fn push_str(&mut self, s: &str) {
        s.chars().for_each(|c| self.push(c));
    }

    fn span(&mut self, kind: Kind, start: i32) {
        self.spans.push(Span { kind, start, end: self.len });
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::enums::ParseMode;
    use crate::raw::base::MessageEntity as E;

    fn run(text: &str, mode: ParseMode) -> (String, Vec<String>) {
        let (text, entities) = parse(text, mode);
        let entities = entities
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
                other => format!("{other:?}"),
            })
            .collect();
        (text, entities)
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
}
