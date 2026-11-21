/// Escapes `text` so [`ParseMode::Html`](crate::enums::ParseMode::Html) reads it back as itself: `&`, `<`, `>` and
/// `"` become entities, so the text can sit inside a tag or in a quoted attribute value.
///
/// ```
/// assert_eq!(vaegram::escape_html("1 < 2 & 3"), "1 &lt; 2 &amp; 3");
/// ```
pub fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

/// Escapes `text` so [`ParseMode::Markdown`](crate::enums::ParseMode::Markdown) and the default mode read it back
/// as itself, with a `\` before every character they treat as markup: ``\ * _ - ~ | ` [ ] < > &``.
///
/// It escapes text, not a link's URL: inside the `(…)` of a link a `)` and a `\` need their own `\`.
///
/// ```
/// assert_eq!(vaegram::escape_markdown("2 * 3 = 6"), "2 \\* 3 = 6");
/// ```
pub fn escape_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\\' | '*' | '_' | '-' | '~' | '|' | '`' | '[' | ']' | '<' | '>' | '&') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{escape_html, escape_markdown};
    use crate::enums::ParseMode;
    use crate::parser::parse;

    /// Every special character of both parsers, next to the plain text and the multi-byte characters around them.
    const NASTY: &str = "a **b** __c__ --d-- ~~e~~ ||f|| `g` ```h``` [i](https://x.y) ![j](tg://emoji?id=1) \\ \\* \
        <b>k</b> &amp; &lt; &#65; > \"q\" 'r' é 😀 \u{a0}\n> quote\n**> expandable||\n- dash\\";

    fn plain(mode: ParseMode, escaped: &str, expected: &str) {
        let parsed = parse(escaped, mode);
        assert_eq!(parsed.as_str(), expected, "{mode:?} {escaped:?}");
        assert!(parsed.write_entities().is_empty(), "{mode:?} {escaped:?}");
    }

    #[test]
    fn html_escapes_read_back_as_the_text() {
        plain(ParseMode::Html, &escape_html(NASTY), NASTY);
        assert_eq!(escape_html("1 < 2 & 3 > \"0\""), "1 &lt; 2 &amp; 3 &gt; &quot;0&quot;");
        assert_eq!(escape_html("plain é 😀"), "plain é 😀");
        // Inside a tag the text stays text.
        let parsed = parse(&format!("<b>{}</b>", escape_html("<i>x</i> & y")), ParseMode::Html);
        assert_eq!(parsed.as_str(), "<i>x</i> & y");
        assert_eq!(parsed.write_entities().len(), 1);
    }

    #[test]
    fn markdown_escapes_read_back_as_the_text() {
        for mode in [ParseMode::Markdown, ParseMode::Default] {
            plain(mode, &escape_markdown(NASTY), NASTY);
            // Inside markup the text stays text.
            let parsed = parse(&format!("**{}**", escape_markdown("*x* _y_ [z](w)")), mode);
            assert_eq!(parsed.as_str(), "*x* _y_ [z](w)", "{mode:?}");
            assert_eq!(parsed.write_entities().len(), 1, "{mode:?}");
        }
        assert_eq!(escape_markdown("a_b * c\\d"), "a\\_b \\* c\\\\d");
        assert_eq!(escape_markdown("plain é 😀."), "plain é 😀.");
    }

    #[test]
    fn every_ascii_character_round_trips() {
        let all: String = (1u8..=126).map(char::from).collect();
        plain(ParseMode::Html, &escape_html(&all), &all);
        for mode in [ParseMode::Markdown, ParseMode::Default] {
            plain(mode, &escape_markdown(&all), &all);
        }
    }
}
