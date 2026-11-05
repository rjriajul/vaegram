use super::rich_block_kind::{button_action, read_style};
use super::rich_text::push_plain;
use crate::raw::base;
use crate::types::{InlineKeyboardButton, RichButtonStyle, RichText};

/// What a piece of received rich text is. Nested text comes as [`RichText`], which has its own `kind`.
///
/// Auto-detected links, emails and phone numbers read as their explicit forms, with the text as the value.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum RichTextKind {
    /// No text.
    Empty,
    /// Text without formatting.
    Plain(String),
    /// Several texts one after another.
    Concat(Vec<RichText>),
    /// Bold text.
    Bold(RichText),
    /// Italic text.
    Italic(RichText),
    /// Underlined text.
    Underline(RichText),
    /// Struck-through text.
    Strikethrough(RichText),
    /// Text hidden behind a spoiler.
    Spoiler(RichText),
    /// Subscript text.
    Subscript(RichText),
    /// Superscript text.
    Superscript(RichText),
    /// Marked (highlighted) text.
    Marked(RichText),
    /// Fixed-width code text.
    Code(RichText),
    /// A link to `url`. Anchor links, which start with `#`, read as [`AnchorLink`](RichTextKind::AnchorLink).
    Url {
        /// Text shown for the link.
        text: RichText,
        /// Address the link points to.
        url: String,
    },
    /// A link to the anchor `name`. An empty name leads back to the top of the message.
    AnchorLink {
        /// Text shown for the link.
        text: RichText,
        /// Name of the anchor, without the leading `#`.
        name: String,
    },
    /// An email address.
    Email {
        /// Text shown for the address.
        text: RichText,
        /// The email address.
        email: String,
    },
    /// A phone number.
    Phone {
        /// Text shown for the number.
        text: RichText,
        /// The phone number.
        phone: String,
    },
    /// A bank card number.
    BankCard(RichText),
    /// A TON address.
    TonAddress(RichText),
    /// A mention of a user by username.
    Mention(RichText),
    /// A hashtag.
    Hashtag(RichText),
    /// A cashtag.
    Cashtag(RichText),
    /// A bot command.
    BotCommand(RichText),
    /// A mention of a user without a username.
    TextMention {
        /// Text shown for the mention.
        text: RichText,
        /// Id of the mentioned user.
        user_id: i64,
    },
    /// A place other text can link to.
    Anchor {
        /// Name of the anchor.
        name: String,
    },
    /// Text that other text can link to.
    Reference {
        /// The text other text links to.
        text: RichText,
        /// Name of the reference.
        name: String,
    },
    /// Inline math holding the formula source.
    Math(String),
    /// A custom emoji.
    CustomEmoji {
        /// Document id of the custom emoji.
        id: i64,
        /// Regular emoji the custom emoji replaces.
        alt: String,
    },
    /// An inline image: a document of the message.
    Image {
        /// Id of the image document.
        document_id: i64,
        /// Width of the image.
        width: i32,
        /// Height of the image.
        height: i32,
    },
    /// A date the client shows in the reader's format. The flags say which parts it shows.
    DateTime {
        /// Fallback text for the date.
        text: RichText,
        /// Unix time being shown.
        date: i32,
        /// Whether the date is shown relative to now.
        relative: bool,
        /// Whether the time is shown in short form.
        short_time: bool,
        /// Whether the time is shown in long form.
        long_time: bool,
        /// Whether the date is shown in short form.
        short_date: bool,
        /// Whether the date is shown in long form.
        long_date: bool,
        /// Whether the day of the week is shown.
        day_of_week: bool,
    },
    /// New text with the text it replaced.
    Diff {
        /// The new text.
        text: RichText,
        /// The text that was replaced.
        old_text: RichText,
    },
    /// A button inside text.
    Button {
        /// Label of the button.
        text: RichText,
        /// What the button does. `None` for a button the keyboard parser can't read, such as a callback whose
        /// data isn't UTF-8.
        action: Option<InlineKeyboardButton>,
        /// Visual style of the button.
        style: RichButtonStyle,
    },
}

impl RichText {
    /// Reads the text into a [`RichTextKind`] describing its outermost formatting.
    pub fn kind(&self) -> RichTextKind {
        use base::RichText as T;
        let wrap = |t: &base::RichText| RichText(t.clone());
        let plain_of = |t: &base::RichText| {
            let mut out = String::new();
            push_plain(t, &mut out);
            out
        };
        match &self.0 {
            T::TextEmpty => RichTextKind::Empty,
            T::TextPlain(t) => RichTextKind::Plain(t.text.clone()),
            T::TextConcat(t) => RichTextKind::Concat(t.texts.iter().map(wrap).collect()),
            T::TextBold(t) => RichTextKind::Bold(wrap(&t.text)),
            T::TextItalic(t) => RichTextKind::Italic(wrap(&t.text)),
            T::TextUnderline(t) => RichTextKind::Underline(wrap(&t.text)),
            T::TextStrike(t) => RichTextKind::Strikethrough(wrap(&t.text)),
            T::TextSpoiler(t) => RichTextKind::Spoiler(wrap(&t.text)),
            T::TextSubscript(t) => RichTextKind::Subscript(wrap(&t.text)),
            T::TextSuperscript(t) => RichTextKind::Superscript(wrap(&t.text)),
            T::TextMarked(t) => RichTextKind::Marked(wrap(&t.text)),
            T::TextFixed(t) => RichTextKind::Code(wrap(&t.text)),
            T::TextUrl(t) => match t.url.strip_prefix('#') {
                Some(name) => RichTextKind::AnchorLink { text: wrap(&t.text), name: name.to_owned() },
                None => RichTextKind::Url { text: wrap(&t.text), url: t.url.clone() },
            },
            T::TextAutoUrl(t) => RichTextKind::Url { text: wrap(&t.text), url: plain_of(&t.text) },
            T::TextEmail(t) => RichTextKind::Email { text: wrap(&t.text), email: t.email.clone() },
            T::TextAutoEmail(t) => RichTextKind::Email { text: wrap(&t.text), email: plain_of(&t.text) },
            T::TextPhone(t) => RichTextKind::Phone { text: wrap(&t.text), phone: t.phone.clone() },
            T::TextAutoPhone(t) => RichTextKind::Phone { text: wrap(&t.text), phone: plain_of(&t.text) },
            T::TextBankCard(t) => RichTextKind::BankCard(wrap(&t.text)),
            T::TextTonAddress(t) => RichTextKind::TonAddress(wrap(&t.text)),
            T::TextMention(t) => RichTextKind::Mention(wrap(&t.text)),
            T::TextHashtag(t) => RichTextKind::Hashtag(wrap(&t.text)),
            T::TextCashtag(t) => RichTextKind::Cashtag(wrap(&t.text)),
            T::TextBotCommand(t) => RichTextKind::BotCommand(wrap(&t.text)),
            T::TextMentionName(t) => RichTextKind::TextMention { text: wrap(&t.text), user_id: t.user_id },
            T::TextAnchor(t) if matches!(t.text, T::TextEmpty) => RichTextKind::Anchor { name: t.name.clone() },
            T::TextAnchor(t) => RichTextKind::Reference { text: wrap(&t.text), name: t.name.clone() },
            T::TextMath(t) => RichTextKind::Math(t.source.clone()),
            T::TextCustomEmoji(t) => RichTextKind::CustomEmoji { id: t.document_id, alt: t.alt.clone() },
            T::TextImage(t) => RichTextKind::Image { document_id: t.document_id, width: t.w, height: t.h },
            T::TextDate(t) => RichTextKind::DateTime {
                text: wrap(&t.text),
                date: t.date,
                relative: t.relative,
                short_time: t.short_time,
                long_time: t.long_time,
                short_date: t.short_date,
                long_date: t.long_date,
                day_of_week: t.day_of_week,
            },
            T::TextDiff(t) => RichTextKind::Diff { text: wrap(&t.text), old_text: wrap(&t.old_text) },
            T::TextButton(t) => RichTextKind::Button {
                text: wrap(&t.text),
                action: button_action(&t.r#type, &plain_of(&t.text)),
                style: read_style(t.style.as_ref()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RichTextKind as K;
    use crate::raw::{base, types};
    use crate::types::{InlineKeyboardButton, RichButtonStyle, RichText};

    fn plain(text: &str) -> base::RichText {
        base::RichText::TextPlain(Box::new(types::TextPlain { text: text.into() }))
    }

    fn kind(raw: base::RichText) -> K {
        RichText(raw).kind()
    }

    #[test]
    fn every_rich_text_has_a_kind() {
        use base::RichText as T;
        let t = || plain("x");
        let wrapped = RichText(t());
        assert_eq!(kind(T::TextEmpty), K::Empty);
        assert_eq!(kind(t()), K::Plain("x".into()));
        let concat = T::TextConcat(Box::new(types::TextConcat { texts: vec![t(), T::TextEmpty] }));
        assert_eq!(kind(concat), K::Concat(vec![wrapped.clone(), RichText(T::TextEmpty)]));
        assert_eq!(kind(T::TextBold(Box::new(types::TextBold { text: t() }))), K::Bold(wrapped.clone()));
        assert_eq!(kind(T::TextItalic(Box::new(types::TextItalic { text: t() }))), K::Italic(wrapped.clone()));
        assert_eq!(kind(T::TextUnderline(Box::new(types::TextUnderline { text: t() }))), K::Underline(wrapped.clone()));
        assert_eq!(kind(T::TextStrike(Box::new(types::TextStrike { text: t() }))), K::Strikethrough(wrapped.clone()));
        assert_eq!(kind(T::TextSpoiler(Box::new(types::TextSpoiler { text: t() }))), K::Spoiler(wrapped.clone()));
        assert_eq!(kind(T::TextSubscript(Box::new(types::TextSubscript { text: t() }))), K::Subscript(wrapped.clone()));
        let sup = T::TextSuperscript(Box::new(types::TextSuperscript { text: t() }));
        assert_eq!(kind(sup), K::Superscript(wrapped.clone()));
        assert_eq!(kind(T::TextMarked(Box::new(types::TextMarked { text: t() }))), K::Marked(wrapped.clone()));
        assert_eq!(kind(T::TextFixed(Box::new(types::TextFixed { text: t() }))), K::Code(wrapped.clone()));
        let url = types::TextUrl { text: t(), url: "https://t.me".into(), webpage_id: 0 };
        assert_eq!(kind(T::TextUrl(Box::new(url))), K::Url { text: wrapped.clone(), url: "https://t.me".into() });
        let link = types::TextUrl { text: t(), url: "#top".into(), webpage_id: 0 };
        assert_eq!(kind(T::TextUrl(Box::new(link))), K::AnchorLink { text: wrapped.clone(), name: "top".into() });
        let email = types::TextEmail { text: t(), email: "a@b.c".into() };
        assert_eq!(kind(T::TextEmail(Box::new(email))), K::Email { text: wrapped.clone(), email: "a@b.c".into() });
        let phone = types::TextPhone { text: t(), phone: "+1".into() };
        assert_eq!(kind(T::TextPhone(Box::new(phone))), K::Phone { text: wrapped.clone(), phone: "+1".into() });
        assert_eq!(kind(T::TextBankCard(Box::new(types::TextBankCard { text: t() }))), K::BankCard(wrapped.clone()));
        let ton = T::TextTonAddress(Box::new(types::TextTonAddress { text: t() }));
        assert_eq!(kind(ton), K::TonAddress(wrapped.clone()));
        assert_eq!(kind(T::TextMention(Box::new(types::TextMention { text: t() }))), K::Mention(wrapped.clone()));
        assert_eq!(kind(T::TextHashtag(Box::new(types::TextHashtag { text: t() }))), K::Hashtag(wrapped.clone()));
        assert_eq!(kind(T::TextCashtag(Box::new(types::TextCashtag { text: t() }))), K::Cashtag(wrapped.clone()));
        let command = T::TextBotCommand(Box::new(types::TextBotCommand { text: t() }));
        assert_eq!(kind(command), K::BotCommand(wrapped.clone()));
        let named = types::TextMentionName { text: t(), user_id: 9 };
        assert_eq!(kind(T::TextMentionName(Box::new(named))), K::TextMention { text: wrapped.clone(), user_id: 9 });
        let anchor = types::TextAnchor { text: T::TextEmpty, name: "a".into() };
        assert_eq!(kind(T::TextAnchor(Box::new(anchor))), K::Anchor { name: "a".into() });
        let reference = types::TextAnchor { text: t(), name: "r".into() };
        assert_eq!(kind(T::TextAnchor(Box::new(reference))), K::Reference { text: wrapped.clone(), name: "r".into() });
        assert_eq!(kind(T::TextMath(Box::new(types::TextMath { source: "x^2".into() }))), K::Math("x^2".into()));
        let emoji = types::TextCustomEmoji { document_id: 5, alt: "🙂".into() };
        assert_eq!(kind(T::TextCustomEmoji(Box::new(emoji))), K::CustomEmoji { id: 5, alt: "🙂".into() });
        assert_eq!(
            kind(T::TextImage(Box::new(types::TextImage { document_id: 6, w: 7, h: 8 }))),
            K::Image { document_id: 6, width: 7, height: 8 }
        );
        let date = types::TextDate {
            relative: false,
            short_time: true,
            long_time: false,
            short_date: false,
            long_date: true,
            day_of_week: true,
            text: t(),
            date: 100,
        };
        let expected = K::DateTime {
            text: wrapped.clone(),
            date: 100,
            relative: false,
            short_time: true,
            long_time: false,
            short_date: false,
            long_date: true,
            day_of_week: true,
        };
        assert_eq!(kind(T::TextDate(Box::new(date))), expected);
        let diff = types::TextDiff { text: t(), old_text: plain("y") };
        assert_eq!(
            kind(T::TextDiff(Box::new(diff))),
            K::Diff { text: wrapped.clone(), old_text: RichText(plain("y")) }
        );
    }

    #[test]
    fn auto_detected_text_reads_as_its_explicit_form() {
        use base::RichText as T;
        let link = T::TextAutoUrl(Box::new(types::TextAutoUrl { text: plain("t.me") }));
        assert_eq!(kind(link), K::Url { text: RichText(plain("t.me")), url: "t.me".into() });
        let mail = T::TextAutoEmail(Box::new(types::TextAutoEmail { text: plain("a@b.c") }));
        assert_eq!(kind(mail), K::Email { text: RichText(plain("a@b.c")), email: "a@b.c".into() });
        let phone = T::TextAutoPhone(Box::new(types::TextAutoPhone { text: plain("+1") }));
        assert_eq!(kind(phone), K::Phone { text: RichText(plain("+1")), phone: "+1".into() });
    }

    #[test]
    fn text_buttons_read_their_action_and_style() {
        use base::RichText as T;
        let url = types::InlineButtonTypeUrl { url: "https://t.me".into() };
        let style = types::RichButtonStyle { bg_primary: false, bg_danger: true, bg_success: false, link: false };
        let button = types::TextButton {
            text: plain("Go"),
            r#type: base::InlineButtonType::InlineButtonTypeUrl(Box::new(url)),
            style: Some(base::RichButtonStyle::RichButtonStyle(style)),
        };
        let K::Button { text, action, style } = kind(T::TextButton(Box::new(button))) else { panic!("not a button") };
        assert_eq!(text.text(), "Go");
        assert_eq!(action, Some(InlineKeyboardButton::url("Go", "https://t.me")));
        assert_eq!(style, RichButtonStyle::Danger);

        let pay =
            types::TextButton { text: plain("Pay"), r#type: base::InlineButtonType::InlineButtonTypeBuy, style: None };
        let K::Button { action, style, .. } = kind(T::TextButton(Box::new(pay))) else { panic!("not a button") };
        assert_eq!((action, style), (Some(InlineKeyboardButton::pay("Pay")), RichButtonStyle::Default));

        let data = types::InlineButtonTypeCallback { requires_password: false, data: vec![0xff, 0xfe] };
        let odd = types::TextButton {
            text: plain("Odd"),
            r#type: base::InlineButtonType::InlineButtonTypeCallback(Box::new(data)),
            style: None,
        };
        let K::Button { action, .. } = kind(T::TextButton(Box::new(odd))) else { panic!("not a button") };
        assert_eq!(action, None);
    }

    #[test]
    fn text_and_emptiness_follow_the_tree() {
        let bold = RichText(base::RichText::TextBold(Box::new(types::TextBold { text: plain("hi") })));
        assert_eq!(bold.text(), "hi");
        assert!(!bold.is_empty());
        assert!(RichText(base::RichText::TextEmpty).is_empty());
    }
}
