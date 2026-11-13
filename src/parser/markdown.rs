use super::html::{Name, Stack, read_markup};
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
    kind: Kind,
    start: i32,
}

/// `html` also reads HTML tags and entities, outside code, with Markdown delimiters and tags sharing one stack.
pub(super) fn parse(src: &str, html: bool) -> Text {
    let mut out = Text::default();
    let mut stack = Stack::default();
    let mut link: Option<Link> = None;
    // Start, and whether a `**>` opened it: only such a quote's own closing `||` makes it expandable. The end is
    // worked out when it closes, so a line that ends in a code span or a fenced block counts like any other.
    let mut quote: Option<(i32, bool)> = None;
    let mut i = 0;
    while i < src.len() {
        let rest = &src[i..];
        if link.as_ref().is_some_and(|l| l.close == i) {
            let l = link.take().unwrap();
            out.span(l.kind, l.start);
            i = l.resume;
            continue;
        }
        // Inside an HTML `code` or `pre` Markdown is plain text, as it is inside a Markdown code span.
        let literal = html && stack.in_code();
        if !literal {
            if i == 0 || src.as_bytes()[i - 1] == b'\n' {
                if let Some(after) = rest.strip_prefix("**>") {
                    if let Some((start, _)) = quote.take() {
                        out.spans.push(Span { kind: Kind::Blockquote(false), start, end: quote_end(&out) });
                    }
                    quote = Some((out.len, true));
                    i += 3 + after.starts_with(' ') as usize;
                    continue;
                }
                if let Some(after) = rest.strip_prefix('>') {
                    quote.get_or_insert((out.len, false));
                    i += 1 + after.starts_with(' ') as usize;
                    continue;
                }
                if let Some((start, _)) = quote.take() {
                    out.spans.push(Span { kind: Kind::Blockquote(false), start, end: quote_end(&out) });
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
            if link.is_none() && rest.starts_with("![") {
                if let Some((close, resume, url)) = find_link(&rest[1..], i + 1) {
                    if let Some(kind) = inline_kind(url) {
                        link = Some(Link { close, resume, kind, start: out.len });
                        i += 2;
                        continue;
                    }
                }
            }
            if link.is_none() && rest.starts_with('[') {
                if let Some((close, resume, url)) = find_link(rest, i) {
                    link = Some(Link { close, resume, kind: Kind::link(url.to_owned()), start: out.len });
                    i += 1;
                    continue;
                }
            }
        }
        if html {
            if let Some(len) = read_markup(rest, &mut stack, &mut out) {
                i += len;
                continue;
            }
        }
        if !literal {
            // A `**>` quote's own closing `||` ends the line it is on: it closes the quote as expandable, and the
            // spoiler that same `||` closes, if there is one. A `||` anywhere else in the quote opens a spoiler.
            if let (Some((start, true)), Some(after)) = (quote, rest.strip_prefix("||")) {
                if after.starts_with('\n') || (after.is_empty() && !stack.has(&Name::Delimiter("||"))) {
                    stack.close(&Name::Delimiter("||"), &mut out);
                    out.spans.push(Span { kind: Kind::Blockquote(true), start, end: quote_end(&out) });
                    quote = None;
                    i += 2;
                    continue;
                }
            }
            if let Some(&delimiter) = DELIMITERS.iter().find(|d| rest.starts_with(**d)) {
                // Telegram entities only nest, so ones opened inside close here and reopen after.
                if stack.close(&Name::Delimiter(delimiter), &mut out) {
                    i += 2;
                    continue;
                }
                if rest[2..].contains(delimiter) {
                    stack.delimiter(delimiter, kind_of(delimiter), &out);
                    i += 2;
                    continue;
                }
            }
        }

        let c = rest.chars().next().unwrap();
        out.push(c);
        i += c.len_utf8();
    }
    if let Some((start, _)) = quote {
        out.spans.push(Span { kind: Kind::Blockquote(false), start, end: quote_end(&out) });
    }
    out
}

/// Where a quote's last line ends: `out` holds its lines and their `\n`, so the span stops before the last one.
fn quote_end(out: &Text) -> i32 {
    if out.text.ends_with('\n') { out.len - 1 } else { out.len }
}

/// The closing `]` and where parsing resumes, as byte indexes offset by `at`, and the URL.
fn find_link(rest: &str, at: usize) -> Option<(usize, usize, &str)> {
    let close = rest.find(']')?;
    let url_part = rest[close + 1..].strip_prefix('(')?;
    let url_end = url_part.find(')')?;
    Some((at + close, at + close + 2 + url_end + 1, &url_part[..url_end]))
}

/// What `![text](url)` stands for: a custom emoji or a date, or `None` for any other URL.
fn inline_kind(url: &str) -> Option<Kind> {
    let number = |s: &str| if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) { s.parse().ok() } else { None };
    if let Some(id) = url.strip_prefix("tg://emoji?id=") {
        return Some(Kind::CustomEmoji(number(id)?));
    }
    let (mut unix, mut format) = (None, "");
    for pair in url.strip_prefix("tg://time?")?.split('&') {
        match pair.split_once('=')? {
            ("unix", value) => unix = Some(i32::try_from(number(value)?).ok()?),
            ("format", value) => format = value,
            _ => return None,
        }
    }
    Some(Kind::date_time(unix?, format))
}

/// Spans the dialect can't express are dropped and their text kept: quotes that don't cover whole lines, code
/// holding its own fence, link, emoji and date text holding `]`, and a span inside one of the same kind. `html`
/// escapes what the mode that also reads HTML would take for a tag or an entity.
pub(super) fn unparse(text: &Text, html: bool) -> String {
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
            Kind::Code if content.contains('`') => continue,
            Kind::Pre(_) if content.contains("```") => continue,
            Kind::Pre(language) if language.contains(|c: char| c.is_whitespace() || c == '`') => language.clear(),
            Kind::Link(_) | Kind::Mention(_) | Kind::CustomEmoji(_) | Kind::DateTime(..) if content.contains(']') => {
                continue;
            }
            _ => {}
        }
        let raw = |k: &Kind| matches!(k, Kind::Code | Kind::Pre(_));
        let class = |k: &Kind| match k {
            Kind::Mention(_) | Kind::CustomEmoji(_) | Kind::DateTime(..) => {
                std::mem::discriminant(&Kind::Link(String::new()))
            }
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

    let mut lines: Vec<(i32, i32, bool)> = Vec::new();
    for q in quotes {
        let whole_lines = (q.start == 0 || char_at(q.start - 1) == Some('\n'))
            && (q.end == text.len || char_at(q.end) == Some('\n'))
            && char_at(q.end - 1) != Some('\n');
        let breaks_code = kept.iter().any(|k| {
            k.start < q.end
                && q.start <= k.end
                && (matches!(k.kind, Kind::Pre(_)) || (k.kind == Kind::Code && inside(k).contains('\n')))
        });
        // The closing `||` would close a spoiler that runs on past the quote instead.
        let expandable = q.kind == Kind::Blockquote(true)
            && !kept.iter().any(|k| k.kind == Kind::Spoiler && k.start <= q.end && q.end < k.end);
        // Only `**>` starts a new quote right after another one; `>` lines would merge into it.
        let joins = !expandable && lines.last().is_some_and(|&(_, end, before)| !before && q.start <= end + 1);
        if whole_lines && !breaks_code && !joins {
            lines.push((q.start, q.end, expandable));
        }
    }

    let mut out =
        Markdown { out: String::with_capacity(text.text.len()), quotes: lines, line: 0, prev: None, raw: 0, html };
    super::walk(&text.text, kept, &mut out);
    if out.quotes.iter().any(|&(_, end, expandable)| expandable && end == text.len) {
        out.out.push_str("||");
    }
    out.out
}

struct Markdown {
    out: String,
    quotes: Vec<(i32, i32, bool)>,
    line: usize,
    prev: Option<char>,
    raw: usize,
    // Whether `<` and `&` need escaping, because the text is read back with HTML tags and entities.
    html: bool,
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
            Kind::Link(_) | Kind::Mention(_) => {
                // `![` would read as an emoji or a date.
                if self.out.ends_with('!') {
                    self.out.insert(self.out.len() - 1, '\\');
                }
                self.out.push('[');
            }
            Kind::CustomEmoji(_) | Kind::DateTime(..) => self.out.push_str("!["),
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
            Kind::CustomEmoji(document_id) => {
                self.out.push_str("](tg://emoji?id=");
                self.out.push_str(&document_id.to_string());
                self.out.push(')');
            }
            &Kind::DateTime(unix_time, flags) => {
                self.out.push_str("](tg://time?unix=");
                self.out.push_str(&unix_time.to_string());
                if flags != 0 {
                    self.out.push_str("&format=");
                    self.out.push_str(&super::date_time_format(flags));
                }
                self.out.push(')');
            }
            kind => self.out.push_str(delimiter(kind)),
        }
    }

    fn char(&mut self, at: i32, c: char) {
        let line_start = matches!(self.prev, None | Some('\n'));
        self.prev = Some(c);
        if line_start {
            if let Some(&(start, _, expandable)) = self.quotes.iter().find(|&&(start, end, _)| start <= at && at < end)
            {
                self.out.insert_str(self.line, if expandable && start == at { "**> " } else { "> " });
            }
        }
        if c == '\n' && self.quotes.iter().any(|&(_, end, expandable)| expandable && end == at) {
            self.out.push_str("||");
        }
        if self.raw > 0 {
            self.out.push(c);
            return;
        }
        let html_syntax = self.html && matches!(c, '<' | '&');
        if matches!(c, '\\' | '*' | '_' | '-' | '~' | '|' | '`' | '[' | ']') || html_syntax || (c == '>' && line_start)
        {
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

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::parser::Kind;

    /// The text of a Markdown parse and its entities as `kind offset length`, sorted, like the tests in `mod.rs`.
    fn run(src: &str) -> (String, Vec<String>) {
        let text = parse(src, false);
        let mut spans = Vec::new();
        for span in &text.spans {
            let length = span.end - span.start;
            let kind = match &span.kind {
                Kind::Blockquote(expandable) => format!("quote:{expandable}"),
                Kind::Spoiler => "spoiler".to_owned(),
                Kind::Code => "code".to_owned(),
                Kind::Pre(language) => format!("pre:{language}"),
                kind => format!("{kind:?}"),
            };
            spans.push(format!("{kind} {} {length}", span.start));
        }
        spans.sort();
        (text.text, spans)
    }

    #[test]
    fn a_quote_ending_in_a_code_span_keeps_its_last_line() {
        // The code span swallows the line's `\n`, so the end can't come from the last newline the loop saw.
        assert_eq!(run("> a `b\n`"), ("a b\n".into(), vec!["code 2 2".into(), "quote:false 0 3".into()]));
    }

    #[test]
    fn a_quote_ending_in_a_fenced_block_keeps_its_last_line() {
        assert_eq!(run("> ```\na\n\n```"), ("a\n".into(), vec!["pre: 0 2".into(), "quote:false 0 1".into()]));
    }

    #[test]
    fn an_expandable_quote_needs_its_own_closing_pipes() {
        assert_eq!(run("**> quoted\nafter"), ("quoted\nafter".into(), vec!["quote:false 0 6".into()]));
        // The pipes here close a spoiler, so the quote never sees its own and stays plain.
        assert_eq!(run("**> ||secret||"), ("secret".into(), vec!["quote:false 0 6".into(), "spoiler 0 6".into()]));
        assert_eq!(
            run("**> first\nplain\n**> second||"),
            ("first\nplain\nsecond".into(), vec!["quote:false 0 5".into(), "quote:true 12 6".into()])
        );
    }

    #[test]
    fn an_expandable_quote_closes_at_its_closing_pipes() {
        assert_eq!(run("**>a\n>b||\nc"), ("a\nb\nc".into(), vec!["quote:true 0 3".into()]));
        assert_eq!(run("x\n**> y||"), ("x\ny".into(), vec!["quote:true 2 1".into()]));
        // One `||` closes the spoiler and the quote both.
        assert_eq!(run("**>||s||\nt"), ("s\nt".into(), vec!["quote:true 0 1".into(), "spoiler 0 1".into()]));
    }
}
