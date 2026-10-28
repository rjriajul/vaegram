use crate::raw::base;

pub(crate) fn push_plain(text: &base::RichText, out: &mut String) {
    use base::RichText as T;
    match text {
        T::TextPlain(t) => out.push_str(&t.text),
        T::TextConcat(t) => t.texts.iter().for_each(|t| push_plain(t, out)),
        T::TextMath(t) => out.push_str(&t.source),
        T::TextCustomEmoji(t) => out.push_str(&t.alt),
        _ => {
            if let Some(inner) = inner(text) {
                push_plain(inner, out);
            }
        }
    }
}

fn inner(text: &base::RichText) -> Option<&base::RichText> {
    use base::RichText as T;
    Some(match text {
        T::TextEmpty
        | T::TextPlain(_)
        | T::TextConcat(_)
        | T::TextImage(_)
        | T::TextMath(_)
        | T::TextCustomEmoji(_) => return None,
        T::TextBold(t) => &t.text,
        T::TextItalic(t) => &t.text,
        T::TextUnderline(t) => &t.text,
        T::TextStrike(t) => &t.text,
        T::TextFixed(t) => &t.text,
        T::TextUrl(t) => &t.text,
        T::TextEmail(t) => &t.text,
        T::TextSubscript(t) => &t.text,
        T::TextSuperscript(t) => &t.text,
        T::TextMarked(t) => &t.text,
        T::TextPhone(t) => &t.text,
        T::TextAnchor(t) => &t.text,
        T::TextSpoiler(t) => &t.text,
        T::TextMention(t) => &t.text,
        T::TextHashtag(t) => &t.text,
        T::TextBotCommand(t) => &t.text,
        T::TextCashtag(t) => &t.text,
        T::TextAutoUrl(t) => &t.text,
        T::TextAutoEmail(t) => &t.text,
        T::TextAutoPhone(t) => &t.text,
        T::TextBankCard(t) => &t.text,
        T::TextMentionName(t) => &t.text,
        T::TextDate(t) => &t.text,
        T::TextDiff(t) => &t.text,
        T::TextButton(t) => &t.text,
        T::TextTonAddress(t) => &t.text,
    })
}
