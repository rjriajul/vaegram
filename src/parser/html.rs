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

pub(super) fn parse(src: &str) -> Text {
    let mut out = Text::default();
    let mut open: Vec<(String, i32, Option<Kind>)> = Vec::new();
    // Tags opened past MAX_NESTING, by name. They are always the innermost ones (every tag opened while some are
    // left goes here too), so a closing tag pairs with them first and never ends an outer tag early.
    let (mut deeper, mut deeper_left) = (HashMap::<String, usize>::new(), 0);
    let mut rest = src;
    while let Some(c) = rest.chars().next() {
        if c == '<' {
            if let Some((tag, len)) = read_tag(rest) {
                rest = &rest[len..];
                // Unknown tags format nothing; kept off the stack, unclosed ones can't use up MAX_NESTING.
                if !is_known(&tag.name) {
                    continue;
                }
                if tag.closing {
                    if let Some(n) = deeper.get_mut(&tag.name).filter(|n| **n > 0) {
                        (*n, deeper_left) = (*n - 1, deeper_left - 1);
                    } else if let Some(i) = open.iter().rposition(|(name, ..)| *name == tag.name) {
                        let (_, start, kind) = open.remove(i);
                        for (_, s, inner) in &mut open[i..] {
                            if let Some(inner) = inner {
                                out.span(inner.clone(), *s);
                            }
                            *s = out.len;
                        }
                        if let Some(kind) = kind {
                            out.span(kind, start);
                        }
                    }
                } else if deeper_left == 0 && open.len() < MAX_NESTING {
                    let kind = kind_of(&tag, &mut open);
                    open.push((tag.name, out.len, kind));
                } else {
                    *deeper.entry(tag.name).or_default() += 1;
                    deeper_left += 1;
                }
                continue;
            }
        } else if c == '&' {
            if let Some((c, len)) = read_entity(rest) {
                out.push(c);
                rest = &rest[len..];
                continue;
            }
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

fn kind_of(tag: &Tag, open: &mut [(String, i32, Option<Kind>)]) -> Option<Kind> {
    Some(match tag.name.as_str() {
        "b" | "strong" => Kind::Bold,
        "i" | "em" => Kind::Italic,
        "u" | "ins" => Kind::Underline,
        "s" | "strike" | "del" => Kind::Strike,
        "tg-spoiler" => Kind::Spoiler,
        "span" if tag.attr("class") == Some("tg-spoiler") => Kind::Spoiler,
        "code" => {
            if let Some((_, _, Some(Kind::Pre(language)))) = open.last_mut() {
                if let Some(lang) = tag.attr("class").and_then(|c| c.strip_prefix("language-")) {
                    *language = lang.to_owned();
                }
                return None;
            }
            Kind::Code
        }
        "pre" => Kind::Pre(String::new()),
        "a" => Kind::link(tag.attr("href")?.to_owned()),
        "blockquote" => Kind::Blockquote(tag.attr("expandable").is_some()),
        "tg-emoji" => Kind::CustomEmoji(tag.attr("emoji-id")?.parse().ok()?),
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
                    let format: String = "rtTdDw"
                        .chars()
                        .enumerate()
                        .filter(|(bit, _)| flags & (1 << bit) != 0)
                        .map(|(_, c)| c)
                        .collect();
                    let _ = write!(self.0, " format=\"{format}\"");
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
