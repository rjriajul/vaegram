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
    // Start, and whether a `**>` opened it, which lets its last line mark it expandable.
    let mut quote: Option<(i32, bool)> = None;
    // Each quote read, and where the line after its last one starts in `out`.
    let mut quotes: Vec<(Span, i32)> = Vec::new();
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
                let expandable = rest.starts_with("**>");
                let continues = !expandable && rest.starts_with('>');
                // A line not starting with `>` ends the quote, and `**>` starts a new one. The `\n` before the line
                // is the quote's line break, not part of its last line.
                if let Some((start, _)) = quote.take_if(|_| !continues) {
                    quotes.push((Span { kind: Kind::Blockquote(false), start, end: out.len - 1 }, out.len));
                }
                if expandable || continues {
                    quote.get_or_insert((out.len, expandable));
                    let marker = if expandable { 3 } else { 1 };
                    i += marker + rest[marker..].starts_with(' ') as usize;
                    continue;
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
            // A `**>` quote is expandable when its last line ends in a `||` that closes no spoiler: that `||` marks
            // it and ends it, so a `>` line after it starts a new quote. A `||` that closes a spoiler is only the
            // spoiler's end, at the end of a line too, so a spoiler that ends an expandable quote is written `||||`.
            if let (Some((start, true)), Some(after)) = (quote, rest.strip_prefix("||")) {
                let line_end = after.is_empty() || after.starts_with('\n');
                if line_end && !stack.has(&Name::Delimiter("||")) {
                    let resume = out.len + i32::from(!after.is_empty());
                    quotes.push((Span { kind: Kind::Blockquote(true), start, end: out.len }, resume));
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
        // A last `\n` in the source is the quote's line break; one that ends a code span or a fenced block is its
        // text, and every other construct ends in its own markup.
        let end = out.len - i32::from(src.ends_with('\n'));
        quotes.push((Span { kind: Kind::Blockquote(false), start, end }, out.len));
    }
    cut_at_quote_ends(&mut out.spans, &quotes);
    out.spans.extend(quotes.into_iter().map(|(quote, _)| quote));
    out
}

/// Cuts each span opened inside a quote that runs on past its end, `(quote, resume)`, so no quote ends inside a span
/// opened in it: the span stops where the quote does and the rest starts at `resume`, past the line break. A custom
/// emoji or a date can't be cut and is dropped.
fn cut_at_quote_ends(spans: &mut Vec<Span>, quotes: &[(Span, i32)]) {
    let crossed = |s: &Span| {
        quotes.iter().find(|(q, _)| q.start <= s.start && s.start < q.end && q.end < s.end).map(|(q, at)| (q.end, *at))
    };
    // Rests are pushed onto the end and checked in turn, as one can run past a later quote too.
    let mut i = 0;
    while i < spans.len() {
        let Some((end, resume)) = crossed(&spans[i]) else {
            i += 1;
            continue;
        };
        if spans[i].kind.is_atomic() {
            spans.remove(i);
            continue;
        }
        let rest = Span { kind: spans[i].kind.clone(), start: resume, end: spans[i].end };
        spans[i].end = end;
        if rest.start < rest.end {
            spans.push(rest);
        }
        i += 1;
    }
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
    use super::{parse, unparse};
    use crate::parser::Kind;
    use crate::types::Text;

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

    #[track_caller]
    fn check(src: &str, text: &str, spans: &[&str]) {
        assert_eq!(run(src), (text.to_owned(), spans.iter().map(|&s| s.to_owned()).collect()), "{src:?}");
    }

    #[test]
    fn a_quote_never_ends_inside_a_code_span_or_a_fenced_block() {
        // The `\n` that ends the code or the block is their text, not the quote's line break.
        check("> a `b\n`", "a b\n", &["code 2 2", "quote:false 0 4"]);
        check("> a `b\n`\nc", "a b\n\nc", &["code 2 2", "quote:false 0 4"]);
        check("> ```\na\n\n```", "a\n", &["pre: 0 2", "quote:false 0 2"]);
        check("**> `a\n`||", "a\n", &["code 0 2", "quote:true 0 2"]);
    }

    #[test]
    fn spans_opened_in_a_quote_stop_where_it_ends() {
        check("> x **a\nb**", "x a\nb", &["Bold 2 1", "Bold 4 1", "quote:false 0 3"]);
        check("> **a\nb**", "a\nb", &["Bold 0 1", "Bold 2 1", "quote:false 0 1"]);
        check("**> x ~~a||\nb~~", "x a\nb", &["Strike 2 1", "Strike 4 1", "quote:true 0 3"]);
        check("> x [a\nb](u)", "x a\nb", &["Link(\"u\") 2 1", "Link(\"u\") 4 1", "quote:false 0 3"]);
        // The rest of a cut span is cut again at the next quote's end.
        check(
            "> a **b\n**> c\nd**",
            "a b\nc\nd",
            &["Bold 2 1", "Bold 4 1", "Bold 6 1", "quote:false 0 3", "quote:false 4 1"],
        );
        // A custom emoji can't be cut, so it goes.
        check("> x ![a\nb](tg://emoji?id=1)", "x a\nb", &["quote:false 0 3"]);
    }

    #[test]
    fn a_quote_is_expandable_when_its_last_line_ends_in_pipes_that_close_no_spoiler() {
        check("**>a\n>b||\nc", "a\nb\nc", &["quote:true 0 3"]);
        check("x\n**> y||", "x\ny", &["quote:true 2 1"]);
        check("**> ||s||||\nt", "s\nt", &["quote:true 0 1", "spoiler 0 1"]);
        check("**> ||secret||\n> more||", "secret\nmore", &["quote:true 0 11", "spoiler 0 6"]);
        check("**> a ||b||\n> c||", "a b\nc", &["quote:true 0 5", "spoiler 2 1"]);
        check("**> first\nplain\n**> second||", "first\nplain\nsecond", &["quote:false 0 5", "quote:true 12 6"]);
        // The mark ends the quote, so a `>` line after it starts a new one.
        check("**> a||\n> b", "a\nb", &["quote:false 2 1", "quote:true 0 1"]);
        check("**> quoted\nafter", "quoted\nafter", &["quote:false 0 6"]);
    }

    #[test]
    fn pipes_that_close_a_spoiler_only_end_the_spoiler() {
        check("**> ||secret||", "secret", &["quote:false 0 6", "spoiler 0 6"]);
        check("**> ||secret||\nplain", "secret\nplain", &["quote:false 0 6", "spoiler 0 6"]);
        check("**> ||secret||\n", "secret\n", &["quote:false 0 6", "spoiler 0 6"]);
        check("**> ||a\n> b||\nc", "a\nb\nc", &["quote:false 0 3", "spoiler 0 3"]);
        // These close the spoiler opened before the quote, which holds the plain quote; the last pipes close
        // nothing and stay text.
        check("||before\n**> q||\n||", "before\nq\n||", &["quote:false 7 1", "spoiler 0 8"]);
    }

    #[test]
    fn an_expandable_quote_ending_in_a_spoiler_round_trips() {
        let text = Text::new().blockquote(Text::new().plain("a ").spoiler("b"), true).plain("\nc");
        let markdown = unparse(&text, false);
        assert_eq!(markdown, "**> a ||b||||\nc");
        check(&markdown, "a b\nc", &["quote:true 0 3", "spoiler 2 1"]);
        for src in ["**> ||s||||\nt", "**> ||secret||\n> more||", "**> a ||b||\n> c||", "**> ||secret||\nplain"] {
            assert_eq!(run(&unparse(&parse(src, false), false)), run(src), "{src:?}");
        }
    }
}
