use super::{Kind, Span};
use crate::types::Text;

const DELIMITERS: [&str; 5] = ["**", "__", "--", "~~", "||"];

fn kind_of(delimiter: &str) -> Kind {
    match delimiter {
        "**" => Kind::Bold,
        "__" => Kind::Italic,
        "--" => Kind::Underline,
        "~~" => Kind::Strike,
        _ => Kind::Spoiler,
    }
}

struct Link {
    close: usize,
    resume: usize,
    url: String,
    start: i32,
}

pub(super) fn parse(src: &str) -> Text {
    let mut out = Text::default();
    let mut open: Vec<(&str, i32)> = Vec::new();
    let mut link: Option<Link> = None;
    let mut quote: Option<(i32, i32)> = None;
    let mut i = 0;
    while i < src.len() {
        let rest = &src[i..];
        if link.as_ref().is_some_and(|l| l.close == i) {
            let l = link.take().unwrap();
            out.span(Kind::link(l.url), l.start);
            i = l.resume;
            continue;
        }
        if i == 0 || src.as_bytes()[i - 1] == b'\n' {
            if let Some(after) = rest.strip_prefix('>') {
                quote.get_or_insert((out.len, out.len));
                i += 1 + after.starts_with(' ') as usize;
                continue;
            }
            if let Some((start, end)) = quote.take() {
                out.spans.push(Span { kind: Kind::Blockquote(false), start, end });
            }
        }

        if let Some(escaped) = rest.strip_prefix('\\').and_then(|r| r.chars().next()) {
            if escaped.is_ascii_punctuation() {
                out.push(escaped);
                i += 2;
                continue;
            }
        }
        if let Some(body) = rest.strip_prefix("```") {
            if let Some(end) = body.find("```") {
                let code = &body[..end];
                let (language, code) = match code.split_once('\n') {
                    Some((lang, code)) if !lang.contains(char::is_whitespace) => (lang, code),
                    _ => ("", code),
                };
                let start = out.len;
                out.push_str(code.strip_suffix('\n').unwrap_or(code));
                out.span(Kind::Pre(language.to_owned()), start);
                i += 6 + end;
                continue;
            }
        }
        if let Some(body) = rest.strip_prefix('`') {
            if let Some(end) = body.find('`') {
                let start = out.len;
                out.push_str(&body[..end]);
                out.span(Kind::Code, start);
                i += 2 + end;
                continue;
            }
        }
        if link.is_none() && rest.starts_with('[') {
            if let Some(l) = find_link(rest, i, out.len) {
                link = Some(l);
                i += 1;
                continue;
            }
        }
        if let Some(&delimiter) = DELIMITERS.iter().find(|d| rest.starts_with(**d)) {
            if let Some(k) = open.iter().rposition(|(d, _)| *d == delimiter) {
                let (_, start) = open.remove(k);
                out.span(kind_of(delimiter), start);
                i += 2;
                continue;
            }
            if rest[2..].contains(delimiter) {
                open.push((delimiter, out.len));
                i += 2;
                continue;
            }
        }

        let c = rest.chars().next().unwrap();
        if let (Some((_, end)), '\n') = (&mut quote, c) {
            *end = out.len;
        }
        out.push(c);
        i += c.len_utf8();
    }
    if let Some((start, end)) = quote {
        let end = if out.text.ends_with('\n') { end } else { out.len };
        out.spans.push(Span { kind: Kind::Blockquote(false), start, end });
    }
    out
}

fn find_link(rest: &str, at: usize, start: i32) -> Option<Link> {
    let close = rest.find(']')?;
    let url_part = rest[close + 1..].strip_prefix('(')?;
    let url_end = url_part.find(')')?;
    Some(Link { close: at + close, resume: at + close + 2 + url_end + 1, url: url_part[..url_end].to_owned(), start })
}
