use std::collections::HashMap;
use std::fmt::Write;

use super::{Kind, MAX_NESTING, Markup};
use crate::types::Text;

fn is_known(name: &str) -> bool {
    matches!(
        name,
        "b" | "strong"
            | "i"
            | "em"
            | "u"
            | "ins"
            | "s"
            | "strike"
            | "del"
            | "tg-spoiler"
            | "span"
            | "code"
            | "pre"
            | "a"
            | "blockquote"
            | "tg-emoji"
            | "emoji"
            | "spoiler"
            | "tg-time"
    )
}

struct Tag {
    name: String,
    closing: bool,
    attrs: Vec<(String, String)>,
}

impl Tag {
    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
}

/// What an open span was opened by: an HTML tag, or a Markdown delimiter in the mode that reads both.
#[derive(PartialEq)]
pub(super) enum Name {
    Tag(String),
    Delimiter(&'static str),
}

struct Open {
    name: Name,
    start: i32,
    kind: Option<Kind>,
}

/// The spans being read and not yet closed, oldest first. Closing one closes and reopens every span opened after
/// it, so the entities come out nested whatever order the markup was closed in.
#[derive(Default)]
pub(super) struct Stack {
    open: Vec<Open>,
    // Tags opened past MAX_NESTING, by name. They are always the innermost ones (every tag opened while some are
    // left goes here too), so a closing tag pairs with them first and never ends an outer tag early.
    deeper: HashMap<String, usize>,
    deeper_left: usize,
}

impl Stack {
    pub(super) fn has(&self, name: &Name) -> bool {
        self.open.iter().any(|o| o.name == *name)
    }

    /// Inside an HTML `code` or `pre`, where Markdown is plain text.
    pub(super) fn in_code(&self) -> bool {
        self.open.iter().any(|o| matches!(o.kind, Some(Kind::Code | Kind::Pre(_))))
    }

    pub(super) fn delimiter(&mut self, delimiter: &'static str, kind: Kind, out: &Text) {
        self.open.push(Open { name: Name::Delimiter(delimiter), start: out.len, kind: Some(kind) });
    }

    /// Ends the innermost open `name`, if there is one.
    pub(super) fn close(&mut self, name: &Name, out: &mut Text) -> bool {
        let Some(i) = self.open.iter().rposition(|o| o.name == *name) else { return false };
        let Open { start, kind, .. } = self.open.remove(i);
        for inner in &mut self.open[i..] {
            if let Some(kind) = &inner.kind {
                out.span(kind.clone(), inner.start);
            }
            inner.start = out.len;
        }
        if let Some(kind) = kind {
            out.span(kind, start);
        }
        true
    }

    fn tag(&mut self, tag: Tag, out: &mut Text) {
        // Unknown tags format nothing; kept off the stack, unclosed ones can't use up MAX_NESTING.
        if !is_known(&tag.name) {
            return;
        }
        if tag.closing {
            if let Some(n) = self.deeper.get_mut(&tag.name).filter(|n| **n > 0) {
                (*n, self.deeper_left) = (*n - 1, self.deeper_left - 1);
            } else {
                self.close(&Name::Tag(tag.name), out);
            }
        } else if self.deeper_left == 0 && self.open.len() < MAX_NESTING {
            let kind = kind_of(&tag, &mut self.open);
            self.open.push(Open { name: Name::Tag(tag.name), start: out.len, kind });
        } else {
            *self.deeper.entry(tag.name).or_default() += 1;
            self.deeper_left += 1;
        }
    }
}

/// Reads the tag or entity `rest` starts with, if it has one, and returns how many bytes it took.
pub(super) fn read_markup(rest: &str, stack: &mut Stack, out: &mut Text) -> Option<usize> {
    match rest.as_bytes().first()? {
        b'<' => {
            let (tag, len) = read_tag(rest)?;
            stack.tag(tag, out);
            Some(len)
        }
        b'&' => {
            let (c, len) = read_entity(rest)?;
            out.push(c);
            Some(len)
        }
        _ => None,
    }
}

pub(super) fn parse(src: &str) -> Text {
    let mut out = Text::default();
    let mut stack = Stack::default();
    let mut rest = src;
    while let Some(c) = rest.chars().next() {
        if let Some(len) = read_markup(rest, &mut stack, &mut out) {
            rest = &rest[len..];
            continue;
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

fn kind_of(tag: &Tag, open: &mut [Open]) -> Option<Kind> {
    Some(match tag.name.as_str() {
        "b" | "strong" => Kind::Bold,
        "i" | "em" => Kind::Italic,
        "u" | "ins" => Kind::Underline,
        "s" | "strike" | "del" => Kind::Strike,
        // `spoiler` and `emoji id=` are alternative spellings of the Bot API's tags.
        "tg-spoiler" | "spoiler" => Kind::Spoiler,
        "span" if tag.attr("class") == Some("tg-spoiler") => Kind::Spoiler,
        "code" => {
            if let Some(Open { kind: Some(Kind::Pre(language)), .. }) = open.last_mut() {
                if let Some(lang) = tag.attr("class").and_then(|c| c.strip_prefix("language-")) {
                    *language = lang.to_owned();
                }
                return None;
            }
            Kind::Code
        }
        "pre" => Kind::Pre(tag.attr("language").unwrap_or_default().to_owned()),
        "a" => Kind::link(tag.attr("href")?.to_owned()),
        "blockquote" => Kind::Blockquote(tag.attr("expandable").is_some()),
        "tg-emoji" => Kind::CustomEmoji(tag.attr("emoji-id")?.parse().ok()?),
        "emoji" => Kind::CustomEmoji(tag.attr("id")?.parse().ok()?),
        "tg-time" => Kind::date_time(tag.attr("unix")?.parse().ok()?, tag.attr("format").unwrap_or_default()),
        _ => return None,
    })
}

fn read_tag(s: &str) -> Option<(Tag, usize)> {
    let after = s[1..].chars().next()?;
    if !after.is_ascii_alphabetic() && after != '/' {
        return None;
    }
    let mut quote = None;
    let end = s.char_indices().skip(1).find_map(|(i, c)| {
        match (quote, c) {
            (None, '>') => return Some(i),
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            _ => {}
        }
        None
    })?;
    let inner = match s[1..end].strip_suffix('/') {
        Some(i) if !i.contains(char::is_whitespace) || i.ends_with([' ', '"', '\'']) => i,
        _ => &s[1..end],
    };
    let (closing, inner) = match inner.strip_prefix('/') {
        Some(i) => (true, i),
        None => (false, inner),
    };
    let name_end = inner.find(char::is_whitespace).unwrap_or(inner.len());
    let name = inner[..name_end].to_ascii_lowercase();
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    Some((Tag { name, closing, attrs: read_attrs(&inner[name_end..]) }, end + 1))
}

fn read_attrs(mut s: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    loop {
        s = s.trim_start();
        let key_end = s.find(|c: char| c == '=' || c.is_whitespace()).unwrap_or(s.len());
        if key_end == 0 {
            return attrs;
        }
        let key = s[..key_end].to_ascii_lowercase();
        s = s[key_end..].trim_start();
        let value = match s.strip_prefix('=').map(str::trim_start) {
            Some(v) => {
                let (value, rest) = match v.chars().next() {
                    Some(q @ ('"' | '\'')) => v[1..].split_once(q).unwrap_or((&v[1..], "")),
                    _ => v.split_at(v.find(char::is_whitespace).unwrap_or(v.len())),
                };
                s = rest;
                decode(value)
            }
            None => String::new(),
        };
        attrs.push((key, value));
    }
}

fn decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        let (c, len) = if c == '&' { read_entity(rest) } else { None }.unwrap_or((c, c.len_utf8()));
        out.push(c);
        rest = &rest[len..];
    }
    out
}

fn read_entity(s: &str) -> Option<(char, usize)> {
    let end = s.bytes().take(12).position(|b| b == b';')?;
    let c = match &s[1..end] {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        name => {
            let n = name.strip_prefix('#')?;
            let code = match n.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => n.parse().ok()?,
            };
            char::from_u32(code)?
        }
    };
    Some((c, end + 1))
}

pub(super) fn unparse(text: &Text) -> String {
    let mut out = Html(String::with_capacity(text.text.len()));
    super::walk(&text.text, text.spans.clone(), &mut out);
    out.0
}

struct Html(String);

impl Html {
    fn attr(&mut self, value: &str) {
        for c in value.chars() {
            match c {
                '&' => self.0.push_str("&amp;"),
                '"' => self.0.push_str("&quot;"),
                c => self.0.push(c),
            }
        }
    }
}

impl Markup for Html {
    fn open(&mut self, kind: &Kind) {
        let tag = match kind {
            Kind::Bold => "<b>",
            Kind::Italic => "<i>",
            Kind::Underline => "<u>",
            Kind::Strike => "<s>",
            Kind::Spoiler => "<tg-spoiler>",
            Kind::Code => "<code>",
            Kind::Pre(language) if language.is_empty() => "<pre>",
            Kind::Pre(language) => {
                self.0.push_str("<pre><code class=\"language-");
                self.attr(language);
                "\">"
            }
            Kind::Link(url) => {
                self.0.push_str("<a href=\"");
                self.attr(url);
                "\">"
            }
            Kind::Mention(user_id) => {
                let _ = write!(self.0, "<a href=\"tg://user?id={user_id}\">");
                ""
            }
            Kind::Blockquote(false) => "<blockquote>",
            Kind::Blockquote(true) => "<blockquote expandable>",
            Kind::CustomEmoji(document_id) => {
                let _ = write!(self.0, "<tg-emoji emoji-id=\"{document_id}\">");
                ""
            }
            &Kind::DateTime(unix_time, flags) => {
                let _ = write!(self.0, "<tg-time unix=\"{unix_time}\"");
                if flags != 0 {
                    let _ = write!(self.0, " format=\"{}\"", super::date_time_format(flags));
                }
                ">"
            }
        };
        self.0.push_str(tag);
    }

    fn close(&mut self, kind: &Kind) {
        self.0.push_str(match kind {
            Kind::Bold => "</b>",
            Kind::Italic => "</i>",
            Kind::Underline => "</u>",
            Kind::Strike => "</s>",
            Kind::Spoiler => "</tg-spoiler>",
            Kind::Code => "</code>",
            Kind::Pre(language) if language.is_empty() => "</pre>",
            Kind::Pre(_) => "</code></pre>",
            Kind::Link(_) | Kind::Mention(_) => "</a>",
            Kind::Blockquote(_) => "</blockquote>",
            Kind::CustomEmoji(_) => "</tg-emoji>",
            Kind::DateTime(..) => "</tg-time>",
        });
    }

    fn char(&mut self, _: i32, c: char) {
        match c {
            '&' => self.0.push_str("&amp;"),
            '<' => self.0.push_str("&lt;"),
            '>' => self.0.push_str("&gt;"),
            c => self.0.push(c),
        }
    }
}
