use super::{Kind, Markup, Span};
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
                // Telegram entities only nest, so ones opened inside close here and reopen after.
                for (d, s) in &mut open[k..] {
                    out.span(kind_of(d), *s);
                    *s = out.len;
                }
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

/// Spans the dialect can't express are dropped and their text kept: custom emoji, dates, quotes that don't cover
/// whole lines, code holding its own fence, link text holding `]`, and a span inside one of the same kind.
pub(super) fn unparse(text: &Text) -> String {
    // Each char's UTF-16 offset and byte index, with the end as the last entry.
    let mut units: Vec<(i32, usize)> = Vec::with_capacity(text.text.len() + 1);
    let mut at = 0;
    for (i, c) in text.text.char_indices() {
        units.push((at, i));
        at += c.len_utf16() as i32;
    }
    units.push((at, text.text.len()));
    let byte = |at: i32| units[units.partition_point(|&(a, _)| a < at).min(units.len() - 1)].1;
    let inside = |s: &Span| &text.text[byte(s.start)..byte(s.end)];
    let char_at = |at: i32| {
        let i = units.binary_search_by_key(&at, |&(a, _)| a).ok()?;
        text.text[units[i].1..].chars().next()
    };
    let mut spans: Vec<Span> =
        text.spans.iter().filter(|s| s.start >= 0 && s.end > s.start && s.end <= text.len).cloned().collect();
    spans.sort_by_key(|s| (s.start, -s.end));

    let (mut kept, mut quotes) = (Vec::<Span>::new(), Vec::<Span>::new());
    for mut s in spans {
        if let Kind::Blockquote(_) = s.kind {
            quotes.push(s);
            continue;
        }
        let content = inside(&s);
        match &mut s.kind {
            Kind::CustomEmoji(_) | Kind::DateTime(..) => continue,
            Kind::Code if content.contains('`') => continue,
            Kind::Pre(_) if content.contains("```") => continue,
            Kind::Pre(language) if language.contains(|c: char| c.is_whitespace() || c == '`') => language.clear(),
            Kind::Link(_) | Kind::Mention(_) if content.contains(']') => continue,
            _ => {}
        }
        let raw = |k: &Kind| matches!(k, Kind::Code | Kind::Pre(_));
        let class = |k: &Kind| match k {
            Kind::Mention(_) => std::mem::discriminant(&Kind::Link(String::new())),
            Kind::Pre(_) => std::mem::discriminant(&Kind::Code),
            k => std::mem::discriminant(k),
        };
        let clashes = kept
            .iter()
            .filter(|k| k.start < s.end && s.start < k.end)
            .any(|k| raw(&k.kind) || class(&k.kind) == class(&s.kind) || (raw(&s.kind) && k.end < s.end));
        if !clashes {
            kept.push(s);
        }
    }

    let mut lines: Vec<(i32, i32)> = Vec::new();
    for q in quotes {
        let whole_lines = (q.start == 0 || char_at(q.start - 1) == Some('\n'))
            && (q.end == text.len || char_at(q.end) == Some('\n'))
            && char_at(q.end - 1) != Some('\n');
        let breaks_code = kept.iter().any(|k| {
            k.start < q.end
                && q.start <= k.end
                && (matches!(k.kind, Kind::Pre(_)) || (k.kind == Kind::Code && inside(k).contains('\n')))
        });
        let joins = lines.last().is_some_and(|&(_, end)| q.start <= end + 1);
        if whole_lines && !breaks_code && !joins {
            lines.push((q.start, q.end));
        }
    }

    let mut out = Markdown { out: String::with_capacity(text.text.len()), quotes: lines, line: 0, prev: None, raw: 0 };
    super::walk(&text.text, kept, &mut out);
    out.out
}

struct Markdown {
    out: String,
    quotes: Vec<(i32, i32)>,
    line: usize,
    prev: Option<char>,
    raw: usize,
}

impl Markup for Markdown {
    fn open(&mut self, kind: &Kind) {
        match kind {
            Kind::Code => {
                self.raw += 1;
                self.out.push('`');
            }
            Kind::Pre(language) => {
                self.raw += 1;
                self.out.push_str("```");
                self.out.push_str(language);
                self.out.push('\n');
            }
            Kind::Link(_) | Kind::Mention(_) => self.out.push('['),
            kind => self.out.push_str(delimiter(kind)),
        }
    }

    fn close(&mut self, kind: &Kind) {
        match kind {
            Kind::Code => {
                self.raw -= 1;
                self.out.push('`');
            }
            Kind::Pre(_) => {
                self.raw -= 1;
                self.out.push_str("\n```");
            }
            Kind::Link(url) => {
                self.out.push_str("](");
                self.out.push_str(&url.replace(')', "%29"));
                self.out.push(')');
            }
            Kind::Mention(user_id) => {
                self.out.push_str("](tg://user?id=");
                self.out.push_str(&user_id.to_string());
                self.out.push(')');
            }
            kind => self.out.push_str(delimiter(kind)),
        }
    }

    fn char(&mut self, at: i32, c: char) {
        let line_start = matches!(self.prev, None | Some('\n'));
        self.prev = Some(c);
        if line_start && self.quotes.iter().any(|&(start, end)| start <= at && at < end) {
            self.out.insert_str(self.line, "> ");
        }
        if self.raw > 0 {
            self.out.push(c);
            return;
        }
        if matches!(c, '\\' | '*' | '_' | '-' | '~' | '|' | '`' | '[' | ']') || (c == '>' && line_start) {
            self.out.push('\\');
        }
        self.out.push(c);
        if c == '\n' {
            self.line = self.out.len();
        }
    }
}

fn delimiter(kind: &Kind) -> &'static str {
    match kind {
        Kind::Bold => "**",
        Kind::Italic => "__",
        Kind::Underline => "--",
        Kind::Strike => "~~",
        Kind::Spoiler => "||",
        _ => "",
    }
}
