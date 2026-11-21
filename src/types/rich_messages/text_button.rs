use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::{ButtonStyle, InlineAction, InlineKeyboardButton, RichButtonStyle, RichText};

/// A button inside text, which converts into [`RichText`]. Read back, it is a
/// [`RichTextKind::Button`](crate::types::RichTextKind::Button).
///
/// ```
/// # fn f() -> vaegram::Result<()> {
/// use vaegram::types::{InlineAction, RichBlock, RichButtonStyle, RichText, TextButton};
///
/// let cell = TextButton::new("e4").action(InlineAction::Callback("move e4".into()))?.style(RichButtonStyle::Primary);
/// let _row = RichBlock::paragraph(vec![RichText::from("Your move: "), cell.into()]);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TextButton {
    text: RichText,
    kind: base::InlineButtonType,
    style: RichButtonStyle,
}

impl TextButton {
    /// Creates a button labelled `text` that does nothing when pressed until [`action`](Self::action) sets what it
    /// does.
    pub fn new(text: impl Into<RichText>) -> Self {
        TextButton {
            text: text.into(),
            kind: base::InlineButtonType::InlineButtonTypeDisabled,
            style: RichButtonStyle::Default,
        }
    }

    /// Sets what pressing the button does.
    ///
    /// Fails with [`Error::InvalidArgument`](crate::Error::InvalidArgument) for an action that names a user (a
    /// profile button, or a login button naming a bot), because it can't be written without looking the user up.
    pub fn action(mut self, action: InlineAction) -> Result<Self> {
        let button = InlineKeyboardButton {
            text: String::new(),
            action,
            style: ButtonStyle::Default,
            icon_custom_emoji_id: None,
        };
        let base::KeyboardInlineButton::KeyboardInlineButton(button) = button.write_offline()?;
        self.kind = button.r#type;
        Ok(self)
    }

    /// Changes the button's color style. [`RichButtonStyle::Default`] sends no style.
    #[must_use]
    pub fn style(mut self, style: RichButtonStyle) -> Self {
        self.style = style;
        self
    }
}

impl From<TextButton> for RichText {
    fn from(button: TextButton) -> Self {
        let TextButton { text, kind, style } = button;
        let button = types::TextButton { text: text.0, r#type: kind, style: style.write() };
        RichText(base::RichText::TextButton(Box::new(button)))
    }
}

#[cfg(test)]
mod tests {
    use super::TextButton;
    use crate::errors::Error;
    use crate::raw::{base, types};
    use crate::types::{InlineAction, InlineKeyboardButton, RichButtonStyle, RichText, RichTextKind, Text};

    #[test]
    fn a_text_button_reads_back_its_label_action_and_style() {
        let button = TextButton::new(Text::new().bold("e4"))
            .action(InlineAction::Callback("move e4".into()))
            .unwrap()
            .style(RichButtonStyle::Success);
        let RichTextKind::Button { text, action, style } = RichText::from(button).kind() else {
            panic!("not a button")
        };
        assert!(matches!(text.kind(), RichTextKind::Bold(_)));
        assert_eq!(action, Some(InlineKeyboardButton::callback("e4", "move e4")));
        assert_eq!(style, RichButtonStyle::Success);
    }

    #[test]
    fn a_new_text_button_is_disabled_and_unstyled() {
        let rich = RichText::from(TextButton::new("x"));
        let base::RichText::TextButton(button) = rich.0 else { panic!("not a button") };
        let expected = types::TextButton {
            text: base::RichText::TextPlain(Box::new(types::TextPlain { text: "x".into() })),
            r#type: base::InlineButtonType::InlineButtonTypeDisabled,
            style: None,
        };
        assert_eq!(*button, expected);
    }

    #[test]
    fn an_action_naming_a_user_is_refused() {
        let e = TextButton::new("me").action(InlineAction::UserProfile(42)).err();
        assert!(matches!(e, Some(Error::InvalidArgument(_))), "{e:?}");
    }
}
