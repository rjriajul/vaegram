use crate::raw::{base, types};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineKeyboardMarkup {
    pub rows: Vec<Vec<InlineKeyboardButton>>,
    pub force_reply: bool,
}

impl InlineKeyboardMarkup {
    pub fn new(rows: impl IntoIterator<Item = Vec<InlineKeyboardButton>>) -> Self {
        InlineKeyboardMarkup { rows: rows.into_iter().collect(), force_reply: false }
    }

    pub fn force_reply(mut self) -> Self {
        self.force_reply = true;
        self
    }

    pub(crate) fn write(&self) -> base::ReplyMarkup {
        let rows = self
            .rows
            .iter()
            .map(|row| {
                let buttons = row.iter().map(InlineKeyboardButton::write).collect();
                base::KeyboardInlineButtonRow::KeyboardInlineButtonRow(Box::new(types::KeyboardInlineButtonRow {
                    buttons,
                }))
            })
            .collect();
        base::ReplyMarkup::ReplyInlineMarkup(Box::new(types::ReplyInlineMarkup { force_reply: self.force_reply, rows }))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ButtonStyle {
    #[default]
    Default,
    Primary,
    Danger,
    Success,
}

impl ButtonStyle {
    pub(crate) fn write(self) -> Option<base::KeyboardButtonStyle> {
        let (bg_primary, bg_danger, bg_success) = match self {
            ButtonStyle::Default => return None,
            ButtonStyle::Primary => (true, false, false),
            ButtonStyle::Danger => (false, true, false),
            ButtonStyle::Success => (false, false, true),
        };
        let style = types::KeyboardButtonStyle { bg_primary, bg_danger, bg_success, icon: None };
        Some(base::KeyboardButtonStyle::KeyboardButtonStyle(Box::new(style)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineKeyboardButton {
    pub text: String,
    pub action: InlineAction,
    pub style: ButtonStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InlineAction {
    Callback(String),
    Url(String),
    SwitchInlineQuery(String),
    SwitchInlineQueryCurrentChat(String),
    CopyText(String),
    Disabled,
    CallbackGame,
}

impl InlineKeyboardButton {
    fn new(text: impl Into<String>, action: InlineAction) -> Self {
        InlineKeyboardButton { text: text.into(), action, style: ButtonStyle::Default }
    }

    pub fn callback(text: impl Into<String>, data: impl Into<String>) -> Self {
        Self::new(text, InlineAction::Callback(data.into()))
    }

    pub fn url(text: impl Into<String>, url: impl Into<String>) -> Self {
        Self::new(text, InlineAction::Url(url.into()))
    }

    pub fn switch_inline_query(text: impl Into<String>, query: impl Into<String>) -> Self {
        Self::new(text, InlineAction::SwitchInlineQuery(query.into()))
    }

    pub fn switch_inline_query_current_chat(text: impl Into<String>, query: impl Into<String>) -> Self {
        Self::new(text, InlineAction::SwitchInlineQueryCurrentChat(query.into()))
    }

    pub fn copy_text(text: impl Into<String>, copy: impl Into<String>) -> Self {
        Self::new(text, InlineAction::CopyText(copy.into()))
    }

    pub fn disabled(text: impl Into<String>) -> Self {
        Self::new(text, InlineAction::Disabled)
    }

    /// Opens the game of a `send_game` message; it has to be the first button of the first row.
    pub fn callback_game(text: impl Into<String>) -> Self {
        Self::new(text, InlineAction::CallbackGame)
    }

    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    fn write(&self) -> base::KeyboardInlineButton {
        use base::InlineButtonType as T;
        let switch = |query: &str, same_peer| {
            T::InlineButtonTypeSwitchInline(Box::new(types::InlineButtonTypeSwitchInline {
                same_peer,
                query: query.into(),
                peer_types: None,
            }))
        };
        let kind = match &self.action {
            InlineAction::Callback(data) => T::InlineButtonTypeCallback(Box::new(types::InlineButtonTypeCallback {
                requires_password: false,
                data: data.as_bytes().to_vec(),
            })),
            InlineAction::Url(url) => T::InlineButtonTypeUrl(Box::new(types::InlineButtonTypeUrl { url: url.clone() })),
            InlineAction::SwitchInlineQuery(q) => switch(q, false),
            InlineAction::SwitchInlineQueryCurrentChat(q) => switch(q, true),
            InlineAction::CopyText(copy) => {
                T::InlineButtonTypeCopy(Box::new(types::InlineButtonTypeCopy { copy_text: copy.clone() }))
            }
            InlineAction::Disabled => T::InlineButtonTypeDisabled,
            InlineAction::CallbackGame => T::InlineButtonTypeGame,
        };
        let button = types::KeyboardInlineButton { style: self.style.write(), text: self.text.clone(), r#type: kind };
        base::KeyboardInlineButton::KeyboardInlineButton(Box::new(button))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::Serializable;

    fn inline(markup: base::ReplyMarkup) -> types::ReplyInlineMarkup {
        match markup {
            base::ReplyMarkup::ReplyInlineMarkup(m) => *m,
            other => panic!("expected replyInlineMarkup, got {other:?}"),
        }
    }

    #[test]
    fn force_reply_maps_to_raw_flag() {
        let plain = InlineKeyboardMarkup::new([vec![InlineKeyboardButton::callback("a", "b")]]);
        assert!(!inline(plain.write()).force_reply);
        let raw = plain.force_reply().write();
        assert!(inline(raw.clone()).force_reply);
        let bytes = raw.to_bytes();
        assert_eq!(bytes[..4], 0xb2b15770u32.to_le_bytes());
        assert_eq!(bytes[4..8], (1u32 << 5).to_le_bytes());
    }

    #[test]
    fn disabled_button_uses_disabled_type() {
        let markup = InlineKeyboardMarkup::new([vec![InlineKeyboardButton::disabled("off")]]);
        let rows = inline(markup.write()).rows;
        let base::KeyboardInlineButtonRow::KeyboardInlineButtonRow(row) = &rows[0];
        let base::KeyboardInlineButton::KeyboardInlineButton(button) = &row.buttons[0];
        assert_eq!(button.text, "off");
        assert_eq!(button.r#type, base::InlineButtonType::InlineButtonTypeDisabled);
        assert_eq!(row.buttons[0].to_bytes()[..4], 0x11c1a322u32.to_le_bytes());
        assert_eq!(button.r#type.to_bytes(), 0xa438619du32.to_le_bytes());
    }
}
