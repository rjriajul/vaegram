use std::cmp::Reverse;

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
    let mut quotes: Vec<Span> = Vec::new();
    // A `[` before this index starts no link: a scan from an earlier one found none, and the scan from any `[` it
    // went past would end at the same place.
    let mut no_link_before = 0;
    let mut i = 0;
    while i < src.len() {
        let rest = &src[i..];
        // What is read whole stops at an open link's `]`, so the link closes there and nothing runs past it.
        let ahead = &src[i..link.as_ref().map_or(src.len(), |l| l.close)];
        // Inside an HTML `code` or `pre` Markdown is plain text, as it is inside a Markdown code span.
        let literal = html && stack.in_code();
        // Before a link's `]` closes it, as the `]` can start a line that ends a quote.
        if !literal && (i == 0 || src.as_bytes()[i - 1] == b'\n') {
            let expandable = rest.starts_with("**>");
            let continues = !expandable && rest.starts_with('>');
            // A line not starting with `>` ends the quote, and `**>` starts a new one. The `\n` before the line
            // is the quote's line break, not part of its last line.
            if let Some((start, _)) = quote.take_if(|_| !continues) {
                quotes.push(Span { kind: Kind::Blockquote(false), start, end: out.len - 1 });
            }
            if expandable || continues {
                quote.get_or_insert((out.len, expandable));
                let marker = if expandable { 3 } else { 1 };
                i += marker + rest[marker..].starts_with(' ') as usize;
                continue;
            }
        }
        if link.as_ref().is_some_and(|l| l.close == i) {
            let l = link.take().unwrap();
            out.span(l.kind, l.start);
            i = l.resume;
            continue;
        }
        if !literal {
            if let Some(escaped) = ahead.strip_prefix('\\').and_then(|r| r.chars().next()) {
                if escaped.is_ascii_punctuation() {
                    out.push(escaped);
                    i += 2;
                    continue;
                }
            }
            if let Some(body) = ahead.strip_prefix("```") {
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
            if let Some(body) = ahead.strip_prefix('`') {
                if let Some(end) = body.find('`') {
                    let start = out.len;
                    out.push_str(&body[..end]);
                    out.span(Kind::Code, start);
                    i += 2 + end;
                    continue;
                }
            }
            // A scan that finds no link says how far no other `[` has one either, unless an HTML tag on the way
            // could hide a backtick or backslash from it, or start text the scan would read as Markdown.
            let reach = |stop: usize| if !html || !src[i..stop].contains('<') { stop } else { 0 };
            if link.is_none() && rest.starts_with("![") && i + 1 >= no_link_before {
                match find_link(src, i + 2) {
                    Ok((close, resume, url)) => {
                        if let Some(kind) = inline_kind(&url) {
                            link = Some(Link { close, resume, kind, start: out.len });
                            i += 2;
                            continue;
                        }
                    }
                    Err(stop) => no_link_before = no_link_before.max(reach(stop)),
                }
            }
            if link.is_none() && rest.starts_with('[') && i >= no_link_before {
                match find_link(src, i + 1) {
                    Ok((close, resume, url)) => {
                        link = Some(Link { close, resume, kind: Kind::link(url), start: out.len });
                        i += 1;
                        continue;
                    }
                    Err(stop) => no_link_before = no_link_before.max(reach(stop)),
                }
            }
        }
        if html {
            if let Some(len) = read_markup(ahead, &mut stack, &mut out) {
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
                    quotes.push(Span { kind: Kind::Blockquote(true), start, end: out.len });
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
        quotes.push(Span { kind: Kind::Blockquote(false), start, end });
    }
    out.spans.extend(quotes);
    uncross(&mut out.spans, &out.text);
    out
}

/// How firmly a span keeps its bounds when two cross: a quote covers whole lines, an emoji or a date can't be cut,
/// and a link cut in two is two links.
fn rank(kind: &Kind) -> u8 {
    match kind {
        Kind::Blockquote(_) => 3,
        Kind::CustomEmoji(_) | Kind::DateTime(..) => 2,
        Kind::Link(_) | Kind::Mention(_) => 1,
        _ => 0,
    }
}

/// Cuts `spans` so no two cross, as Telegram entities only nest. Of two that cross, the one of lower rank, else the
/// one starting later, is cut where the other starts or ends, and an emoji or a date that would be cut is dropped. A
/// span cut at a quote's edge leaves out the line break beside the quote, and so does every span starting at that
/// break after the quote or ending at it before the quote, so the spans still nest. Empty spans are dropped.
fn uncross(spans: &mut Vec<Span>, text: &str) {
    if spans.iter().all(|s| rank(&s.kind) == 0) {
        spans.retain(|s| s.start < s.end);
        return;
    }
    // Spans of rank 0 nest among themselves as read (the stack closes and reopens them, code is read whole), so they
    // go last and are only checked against the firm ones.
    let mut order: Vec<usize> = (0..spans.len()).collect();
    order.sort_by_key(|&i| (Reverse(rank(&spans[i].kind)), spans[i].start));
    // The firm spans kept, and whether each is a quote.
    let mut firm: Vec<(i32, i32, bool)> = Vec::new();
    // Where spans were cut at a quote's end, and at a quote's start.
    let (mut cuts, mut after, mut before) = (Vec::new(), Vec::new(), Vec::new());
    for i in order {
        let (start, end) = (spans[i].start, spans[i].end);
        let (is_firm, is_quote) = (rank(&spans[i].kind) > 0, matches!(spans[i].kind, Kind::Blockquote(_)));
        let atomic = spans[i].kind.is_atomic();
        cuts.clear();
        for &(f_start, f_end, quote) in &firm {
            if f_start < start && start < f_end && f_end < end {
                cuts.push(f_end);
                if quote && !atomic {
                    after.push(f_end);
                }
            } else if start < f_start && f_start < end && end < f_end {
                cuts.push(f_start);
                if quote && !atomic {
                    before.push(f_start);
                }
            }
        }
        if atomic && !cuts.is_empty() {
            spans[i].end = start;
            continue;
        }
        cuts.sort_unstable();
        cuts.dedup();
        let mut from = start;
        for to in cuts.iter().copied().chain([end]) {
            if from == start {
                spans[i].end = to;
            } else {
                spans.push(Span { kind: spans[i].kind.clone(), start: from, end: to });
            }
            if is_firm {
                firm.push((from, to, is_quote));
            }
            from = to;
        }
    }
    if !after.is_empty() || !before.is_empty() {
        let mut at = 0;
        let breaks: Vec<i32> = text
            .chars()
            .filter_map(|c| {
                let here = at;
                at += c.len_utf16() as i32;
                (c == '\n').then_some(here)
            })
            .collect();
        let is_break = |at: i32| breaks.binary_search(&at).is_ok();
        for moves in [&mut after, &mut before] {
            moves.sort_unstable();
            moves.dedup();
        }
        for &at in after.iter().filter(|&&at| is_break(at)) {
            spans.iter_mut().filter(|s| s.start == at).for_each(|s| s.start += 1);
        }
        for &at in before.iter().filter(|&&at| is_break(at - 1)) {
            spans.iter_mut().filter(|s| s.end == at).for_each(|s| s.end -= 1);
        }
    }
    spans.retain(|s| s.start < s.end);
}

/// Reads link text from `from` as `parse` reads it, so an escaped `]` or one in code doesn't end it, then the `(url)`
/// after its `]`. Gives the index of that `]`, where parsing resumes and the URL, in which `\` escapes the next
/// ASCII character as in Telegram's MarkdownV2. A `[` on the way starts a link of its own, as in MarkdownV2, so this
/// one is none. With no link, gives where the scan stopped, which the scan from any `[` it went past would reach too.
fn find_link(src: &str, from: usize) -> Result<(usize, usize, String), usize> {
    let bytes = src.as_bytes();
    let mut close = from;
    loop {
        match bytes.get(close) {
            None => return Err(src.len()),
            Some(b']') => break,
            Some(b'[') => return Err(close),
            Some(b'\\') if src[close + 1..].starts_with(|c: char| c.is_ascii_punctuation()) => close += 2,
            Some(b'`') => {
                let rest = &src[close..];
                let fenced = rest.strip_prefix("```").and_then(|body| body.find("```")).map(|end| 6 + end);
                close += fenced.or_else(|| rest[1..].find('`').map(|end| 2 + end)).unwrap_or(1);
            }
            _ => close += 1,
        }
    }
    if bytes.get(close + 1) != Some(&b'(') {
        return Err(close);
    }
    let (mut url, mut at) = (String::new(), close + 2);
    while let Some(c) = src[at..].chars().next() {
        match c {
            ')' => return Ok((close, at + 1, url)),
            '\\' if bytes.get(at + 1).is_some_and(|b| (1..=126).contains(b)) => {
                url.push(char::from(bytes[at + 1]));
                at += 2;
            }
            c => {
                url.push(c);
                at += c.len_utf8();
            }
        }
    }
    Err(src.len())
}

/// What `![text](url)` stands for: a custom emoji or a date, or `None` for any other URL.
fn inline_kind(url: &str) -> Option<Kind> {
    // Signed: document ids and times come from Telegram as signed integers.
    let number = |s: &str| {
        let digits = s.strip_prefix('-').unwrap_or(s);
        if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) { s.parse().ok() } else { None }
    };
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

/// Writes `text` so `parse` reads it back, except where the dialect can't say it. These spans are dropped, their text
/// kept: empty ones, ones starting before the text or ending past it; quotes that don't cover whole lines (one ending
/// in a line break doesn't), a quote inside or across another, and one touching a fenced block or a code span holding
/// a line break; code holding a backtick and a fenced block holding three; a span inside or across code or a fenced
/// block (one with just its range is written around it, but of a code span and a fenced block with the same range
/// only the first is kept); and a span inside or across one of the same kind, mentions, emoji and dates counting as
/// links. Spans that cross are written nested, so the one opened later comes back in two pieces, cut where the other
/// ends. When one of the two is a quote, the other is the one cut, at the quote's edge, and its pieces leave out the
/// line break beside the quote. These change: a fenced block's language holding whitespace or three backticks is
/// cleared; an expandable quote that ends inside a spoiler is written plain; a spoiler starting at a line break in a
/// quote written with `**>` (an expandable one, or one right after another quote), or at the break right after that
/// quote, starts after the break; and a link to `tg://user?id=…` comes back as a mention. `html` escapes what the
/// mode that also reads HTML would take for a tag or an entity.
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
    let raw = |k: &Kind| matches!(k, Kind::Code | Kind::Pre(_));
    let mut spans: Vec<Span> =
        text.spans.iter().filter(|s| s.start >= 0 && s.end > s.start && s.end <= text.len).cloned().collect();
    // A span with just the range of a code span or a fenced block goes first, to be written around it.
    spans.sort_by_key(|s| (s.start, -s.end, raw(&s.kind)));

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
            // The first line would be read as code, or the block would end in the language.
            Kind::Pre(language) if language.contains(char::is_whitespace) || language.contains("```") => {
                language.clear();
            }
            _ => {}
        }
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

    let mut lines: Vec<Quote> = Vec::new();
    for q in quotes {
        let whole_lines = (q.start == 0 || char_at(q.start - 1) == Some('\n'))
            && (q.end == text.len || char_at(q.end) == Some('\n'))
            && char_at(q.end - 1) != Some('\n');
        let breaks_code = kept.iter().any(|k| {
            k.start < q.end
                && q.start <= k.end
                && (matches!(k.kind, Kind::Pre(_)) || (k.kind == Kind::Code && inside(k).contains('\n')))
        });
        // The closing `||` would close a spoiler that runs on past the quote instead. One that starts at the quote's
        // end starts after the line break there, below.
        let expandable = q.kind == Kind::Blockquote(true)
            && !kept.iter().any(|k| k.kind == Kind::Spoiler && k.start < q.end && q.end < k.end);
        let last = lines.last();
        // Quotes don't nest. The lines are in order and apart, so the last one reaches furthest.
        let nested = last.is_some_and(|l| q.start < l.end);
        // A `>` line right after a quote its mark doesn't end would merge into it; `**>` starts a new one.
        let starred = expandable || last.is_some_and(|l| !l.expandable && q.start <= l.end + 1);
        if whole_lines && !breaks_code && !nested {
            lines.push(Quote { start: q.start, end: q.end, starred, expandable });
        }
    }
    // A spoiler's `||` just before a line break in a quote started by `**>`, or the one after it, would read as the
    // mark that makes the quote expandable and ends it.
    for k in kept.iter_mut().filter(|k| k.kind == Kind::Spoiler && char_at(k.start) == Some('\n')) {
        if lines.iter().any(|l| l.starred && l.start <= k.start && k.start <= l.end) {
            k.start += 1;
        }
    }

    let mut out = Markdown {
        out: String::with_capacity(text.text.len()),
        quotes: lines,
        next: 0,
        line: 0,
        prev: None,
        raw: 0,
        html,
    };
    super::walk(&text.text, kept, &mut out);
    // The quotes are in order and apart, so the last one ends last.
    if out.quotes.last().is_some_and(|q| q.expandable && q.end == text.len) {
        out.out.push_str("||");
    }
    out.out
}

/// A quote as written: whether its first line starts with `**>` and whether `||` marks its last one.
struct Quote {
    start: i32,
    end: i32,
    starred: bool,
    expandable: bool,
}

struct Markdown {
    out: String,
    quotes: Vec<Quote>,
    // The first quote not ended before the char being written.
    next: usize,
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
                for c in url.chars() {
                    if matches!(c, '\\' | ')') {
                        self.out.push('\\');
                    }
                    self.out.push(c);
                }
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
        while self.quotes.get(self.next).is_some_and(|q| q.end < at) {
            self.next += 1;
        }
        // Quotes are apart, so only that one can hold this char or end at it.
        let quote = self.quotes.get(self.next);
        if line_start {
            if let Some(q) = quote.filter(|q| q.start <= at && at < q.end) {
                self.out.insert_str(self.line, if q.starred && q.start == at { "**> " } else { "> " });
            }
        }
        if c == '\n' && quote.is_some_and(|q| q.expandable && q.end == at) {
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
        check("**> x ~~a||\nb~~", "x a\nb", &["Strike 2 1", "Strike 4 1", "quote:true 0 3"]);
        check("> x [a\nb](u)", "x a\nb", &["Link(\"u\") 2 1", "Link(\"u\") 4 1", "quote:false 0 3"]);
        // Its rest starts where the next quote does and holds it whole.
        check("> a **b\n**> c\nd**", "a b\nc\nd", &["Bold 2 1", "Bold 4 3", "quote:false 0 3", "quote:false 4 1"]);
        // A custom emoji can't be cut, so it goes.
        check("> x ![a\nb](tg://emoji?id=1)", "x a\nb", &["quote:false 0 3"]);
    }

    #[test]
    fn a_span_holding_a_whole_quote_is_left_whole() {
        // Opened where the quote starts, it holds the quote rather than crossing its end.
        check("> **a\nb**", "a\nb", &["Bold 0 3", "quote:false 0 1"]);
        check("**> **a||\nb**", "a\nb", &["Bold 0 3", "quote:true 0 1"]);
    }

    #[test]
    fn a_span_running_into_a_quote_stops_before_its_line() {
        check("**a\n> b** c", "a\nb c", &["Bold 0 1", "Bold 2 1", "quote:false 2 3"]);
        check("||before\n> q|| more", "before\nq more", &["quote:false 7 6", "spoiler 0 6", "spoiler 7 1"]);
        check("[a\n> b](u) c", "a\nb c", &["Link(\"u\") 0 1", "Link(\"u\") 2 1", "quote:false 2 3"]);
        // It holds the expandable quote whole and stops before the line of the next one.
        check("~~a\n**> b||\n> c~~ d", "a\nb\nc d", &["Strike 0 3", "Strike 4 1", "quote:false 4 3", "quote:true 2 1"]);
    }

    #[test]
    fn a_span_crossing_a_link_is_cut_at_the_link_edge() {
        check("[a **b](u) c**", "a b c", &["Bold 2 1", "Bold 3 2", "Link(\"u\") 0 3"]);
        check("**a [b** c](u)", "a b c", &["Bold 0 2", "Bold 2 1", "Link(\"u\") 2 3"]);
        // An emoji is cut by nothing: the span is cut instead.
        check("![a **b](tg://emoji?id=1) c**", "a b c", &["Bold 2 1", "Bold 3 2", "CustomEmoji(1) 0 3"]);
    }

    #[test]
    fn a_link_closing_at_the_start_of_a_line_ends_the_quote() {
        check("> [a\n](u) b", "a\n b", &["Link(\"u\") 0 2", "quote:false 0 1"]);
        check("> x [a\n](u) b\n> c", "x a\n b\nc", &["Link(\"u\") 2 1", "quote:false 0 3", "quote:false 7 1"]);
    }

    #[test]
    fn spans_starting_at_a_cut_line_break_move_with_the_cut() {
        // The bold is cut at the quote's end and resumes after the line break, so the italic opened at that
        // break starts after it too, inside the bold's rest.
        check("> x **a__\nb__ c**", "x a\nb c", &["Bold 2 1", "Bold 4 3", "Italic 4 1", "quote:false 0 3"]);
    }

    #[test]
    fn an_escaped_bracket_or_one_in_code_does_not_close_a_link() {
        // The `[` that swallowed nothing stays text, and the link after it is read.
        check(r"[a\](u) [b](v)", "[a](u) b", &["Link(\"v\") 7 1"]);
        check(r"![a\](tg://emoji?id=1) [b](v)", "![a](tg://emoji?id=1) b", &["Link(\"v\") 22 1"]);
        check("[a `b](u)` c](v) [d](w)", "a b](u) c d", &["Link(\"v\") 0 9", "Link(\"w\") 10 1", "code 2 5"]);
    }

    #[test]
    fn a_bracket_that_opens_no_link_stays_text() {
        // A `[` in link text starts a link of its own.
        check("[a [b](v)", "[a b", &["Link(\"v\") 3 1"]);
        check("[[[[a](u)", "[[[a", &["Link(\"u\") 3 1"]);
        check("[a](u", "[a](u", &[]);
    }

    #[test]
    fn a_backslash_in_a_url_escapes_the_next_character() {
        check(r"[a](x\) [b](v)", "a", &["Link(\"x) [b](v\") 0 1"]);
        check(r"[a](x\\y\)z\b)", "a", &["Link(\"x\\\\y)zb\") 0 1"]);
    }

    #[test]
    fn a_span_with_the_range_of_code_is_written_around_it() {
        let cases = [
            ("**`abc`**", "abc", ["Bold 0 3", "code 0 3"], "**`abc`**"),
            ("**```a```**", "a", ["Bold 0 1", "pre: 0 1"], "**```\na\n```**"),
            ("[`abc`](u)", "abc", ["Link(\"u\") 0 3", "code 0 3"], "[`abc`](u)"),
        ];
        for (src, text, spans, written) in cases {
            check(src, text, &spans);
            assert_eq!(unparse(&parse(src, false), false), written);
        }
    }

    #[test]
    fn empty_spans_are_dropped() {
        check("****", "", &[]);
        check("a ____ b", "a  b", &[]);
        assert_eq!(parse(&unparse(&parse("****", false), false), false), parse("****", false));
    }

    /// The text and its spans in a fixed order.
    fn normal(text: &Text) -> (String, Vec<String>) {
        let mut spans: Vec<String> = text.spans.iter().map(|s| format!("{:?} {} {}", s.kind, s.start, s.end)).collect();
        spans.sort();
        (text.text.clone(), spans)
    }

    #[test]
    fn parsed_spans_never_cross_and_survive_a_round_trip() {
        let inputs = [
            "**a\n> b** c",
            "||before\n> q|| more",
            "[a **b](u) c**",
            "> [a\n](u) b",
            "**a [b** c](u)",
            "> x **a__\nb__ c**",
            "> a **b\n**> c\nd**",
            "> x [a\nb](u)",
            "> x ![a\nb](tg://emoji?id=1)",
            "**> x ~~a||\nb~~",
            "> **a\nb**",
            "**> **a||\nb**",
            "||before\n**> q||\n||",
            "**> ||a\n> b||\nc",
            "**> a ||b||||\nc",
            "**ab __cd** ef__",
            "[a __b](u) c__ **d [e** f](v)",
            "__a\n> b [c\n> d](u) e__ f",
            "> **a\n> b** c\nd",
            "x **y\n> z\n> w** v\nu",
            "**a\n**> b** c||",
            "||x\n**> a|| b",
            "> a [b\nc **d](u) e**",
            "![x **y](tg://emoji?id=1) z**",
            "**a ![b** c](tg://time?unix=5)",
            "~~a\n**> b||\n> c~~ d",
            "> a\n> b\n**c\n> d** e",
            "[a\n> b](u)",
            "--a [b\n> c](u) d--",
            "> a `b`\n> **c\nd**",
            "> a\n**> b ||c\n> d|| e\n> f",
            "**> a ||b\n> c||||\n**> d\n> e",
            "**a\n> `b` c** d",
            "😀**a\n> 😀** b",
            "[**a](u)** b",
            "> 😀 [é **😀\n](u) b** c",
            "**a <i>b\n> c</i> d** e",
            "[a <i>b](u) c</i>",
            "<b>a\n> b</b> c",
            "> a <u>b\nc</u>",
            "<b>a [b</b> c](u)",
            "[a <code>b](u) c</code>",
            "**`abc`**",
            "[`abc`](u)",
            "**```a```**",
            r"[a\](u) [b](v)",
            r"![a\](tg://emoji?id=1) [b](v)",
            "[a `b](u)` c](v) [d](w)",
            "****",
            "a ____ b",
            "[a [b](v)",
            "[[[[a](u)",
            r"[a](x\) [b](v)",
            r"[a](x\\y\)z\b)",
            "![e](tg://emoji?id=-5)",
            "```a`b\nx```",
        ];
        for html in [false, true] {
            for src in inputs {
                let text = parse(src, html);
                for a in &text.spans {
                    assert!(0 <= a.start && a.start < a.end && a.end <= text.len, "{src:?} html={html}: {a:?}");
                    for b in &text.spans {
                        let crosses = a.start < b.start && b.start < a.end && a.end < b.end;
                        assert!(!crosses, "{src:?} html={html}: {a:?} crosses {b:?}");
                    }
                }
                let markdown = unparse(&text, html);
                assert_eq!(normal(&parse(&markdown, html)), normal(&text), "{src:?} html={html} became {markdown:?}");
            }
        }
    }

    #[test]
    fn nested_quotes_keep_only_the_first() {
        let identical = Text::new().blockquote(Text::new().blockquote("a", false), true);
        assert_eq!(unparse(&identical, false), "> a");
        let identical = Text::new().blockquote(Text::new().blockquote("a", true), false);
        assert_eq!(unparse(&identical, false), "**> a||");
        let inner = Text::new().blockquote(Text::new().plain("a\n").blockquote("b", true), false).plain("\nc");
        let markdown = unparse(&inner, false);
        assert_eq!(markdown, "> a\n> b\nc");
        check(&markdown, "a\nb\nc", &["quote:false 0 3"]);
    }

    #[test]
    fn a_spoiler_starting_at_a_line_break_of_an_expandable_quote_starts_after_it() {
        let inside = Text::new().blockquote(Text::new().plain("a").spoiler("\nb"), true);
        let markdown = unparse(&inside, false);
        assert_eq!(markdown, "**> a\n> ||b||||");
        check(&markdown, "a\nb", &["quote:true 0 3", "spoiler 2 1"]);
        let after = Text::new().blockquote("a", true).spoiler("\nb");
        let markdown = unparse(&after, false);
        assert_eq!(markdown, "**> a||\n||b||");
        check(&markdown, "a\nb", &["quote:true 0 1", "spoiler 2 1"]);
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
