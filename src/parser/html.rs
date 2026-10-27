use super::{Kind, Out};

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

pub(super) fn parse(src: &str) -> Out {
    let mut out = Out::default();
    let mut open: Vec<(String, i32, Option<Kind>)> = Vec::new();
    let mut rest = src;
    while let Some(c) = rest.chars().next() {
        if c == '<' {
            if let Some((tag, len)) = read_tag(rest) {
                rest = &rest[len..];
                if tag.closing {
                    if let Some(i) = open.iter().rposition(|(name, ..)| *name == tag.name) {
                        let (_, start, kind) = open.remove(i);
                        if let Some(kind) = kind {
                            out.span(kind, start);
                        }
                    }
                } else {
                    let kind = kind_of(&tag, &mut open);
                    open.push((tag.name, out.len, kind));
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
        "a" => Kind::Link(tag.attr("href")?.to_owned()),
        "blockquote" => Kind::Blockquote(tag.attr("expandable").is_some()),
        "tg-emoji" => Kind::CustomEmoji(tag.attr("emoji-id")?.parse().ok()?),
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
