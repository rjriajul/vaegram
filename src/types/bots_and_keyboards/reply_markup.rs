use crate::raw::base;
use crate::types::{
    ButtonStyle, ForceReply, InlineAction, InlineKeyboardButton, InlineKeyboardMarkup, KeyboardButton,
    KeyboardButtonRequestManagedBot, ReplyKeyboardMarkup, ReplyKeyboardRemove,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplyMarkup {
    Inline(InlineKeyboardMarkup),
    Keyboard(ReplyKeyboardMarkup),
    Remove(ReplyKeyboardRemove),
    ForceReply(ForceReply),
}

impl ReplyMarkup {
    pub(crate) fn write(&self) -> base::ReplyMarkup {
        match self {
            ReplyMarkup::Inline(m) => m.write(),
            ReplyMarkup::Keyboard(m) => m.write(),
            ReplyMarkup::Remove(m) => m.write(),
            ReplyMarkup::ForceReply(m) => m.write(),
        }
    }

    pub(crate) fn parse(raw: &base::ReplyMarkup) -> Option<ReplyMarkup> {
        Some(match raw {
            base::ReplyMarkup::ReplyKeyboardHide(m) => {
                ReplyMarkup::Remove(ReplyKeyboardRemove { selective: m.selective })
            }
            base::ReplyMarkup::ReplyKeyboardForceReply(m) => {
                ReplyMarkup::ForceReply(ForceReply { selective: m.selective, placeholder: m.placeholder.clone() })
            }
            base::ReplyMarkup::ReplyKeyboardMarkup(m) => {
                let rows = m.rows.iter().map(keyboard_row).collect::<Option<_>>()?;
                ReplyMarkup::Keyboard(ReplyKeyboardMarkup {
                    rows,
                    resize: m.resize,
                    one_time: m.single_use,
                    persistent: m.persistent,
                    selective: m.selective,
                    force_reply: m.force_reply,
                    placeholder: m.placeholder.clone(),
                })
            }
            base::ReplyMarkup::ReplyInlineMarkup(m) => {
                let rows = m.rows.iter().map(inline_row).collect::<Option<_>>()?;
                ReplyMarkup::Inline(InlineKeyboardMarkup { rows, force_reply: m.force_reply })
            }
        })
    }
}

fn keyboard_row(raw: &base::KeyboardButtonRow) -> Option<Vec<KeyboardButton>> {
    let base::KeyboardButtonRow::KeyboardButtonRow(row) = raw;
    row.buttons.iter().map(keyboard_button).collect()
}

fn keyboard_button(raw: &base::KeyboardButton) -> Option<KeyboardButton> {
    let base::KeyboardButton::KeyboardButton(b) = raw;
    let mut button = KeyboardButton::new(b.text.clone()).style(style(b.style.as_ref()));
    match &b.r#type {
        base::ButtonType::ButtonTypeDefault => {}
        base::ButtonType::ButtonTypeRequestPhone => button.request_contact = true,
        base::ButtonType::ButtonTypeRequestGeoLocation => button.request_location = true,
        base::ButtonType::ButtonTypeRequestPeer(p) => {
            let base::RequestPeerType::RequestPeerTypeCreateBot(bot) = &p.peer_type else { return None };
            button.request_managed_bot = Some(KeyboardButtonRequestManagedBot {
                request_id: p.button_id,
                suggested_name: bot.suggested_name.clone(),
                suggested_username: bot.suggested_username.clone(),
            });
        }
        _ => return None,
    }
    Some(button)
}

fn inline_row(raw: &base::KeyboardInlineButtonRow) -> Option<Vec<InlineKeyboardButton>> {
    let base::KeyboardInlineButtonRow::KeyboardInlineButtonRow(row) = raw;
    row.buttons.iter().map(inline_button).collect()
}

fn inline_button(raw: &base::KeyboardInlineButton) -> Option<InlineKeyboardButton> {
    use base::InlineButtonType as T;
    let base::KeyboardInlineButton::KeyboardInlineButton(b) = raw;
    let action = match &b.r#type {
        T::InlineButtonTypeCallback(t) if !t.requires_password => {
            InlineAction::Callback(std::str::from_utf8(&t.data).ok()?.to_owned())
        }
        T::InlineButtonTypeUrl(t) => InlineAction::Url(t.url.clone()),
        T::InlineButtonTypeSwitchInline(t) if t.peer_types.is_none() && t.same_peer => {
            InlineAction::SwitchInlineQueryCurrentChat(t.query.clone())
        }
        T::InlineButtonTypeSwitchInline(t) if t.peer_types.is_none() => {
            InlineAction::SwitchInlineQuery(t.query.clone())
        }
        T::InlineButtonTypeCopy(t) => InlineAction::CopyText(t.copy_text.clone()),
        T::InlineButtonTypeDisabled => InlineAction::Disabled,
        T::InlineButtonTypeGame => InlineAction::CallbackGame,
        _ => return None,
    };
    Some(InlineKeyboardButton { text: b.text.clone(), action, style: style(b.style.as_ref()) })
}

fn style(raw: Option<&base::KeyboardButtonStyle>) -> ButtonStyle {
    match raw {
        Some(base::KeyboardButtonStyle::KeyboardButtonStyle(s)) if s.bg_primary => ButtonStyle::Primary,
        Some(base::KeyboardButtonStyle::KeyboardButtonStyle(s)) if s.bg_danger => ButtonStyle::Danger,
        Some(base::KeyboardButtonStyle::KeyboardButtonStyle(s)) if s.bg_success => ButtonStyle::Success,
        _ => ButtonStyle::Default,
    }
}

impl From<InlineKeyboardMarkup> for ReplyMarkup {
    fn from(m: InlineKeyboardMarkup) -> Self {
        ReplyMarkup::Inline(m)
    }
}

impl From<ReplyKeyboardMarkup> for ReplyMarkup {
    fn from(m: ReplyKeyboardMarkup) -> Self {
        ReplyMarkup::Keyboard(m)
    }
}

impl From<ReplyKeyboardRemove> for ReplyMarkup {
    fn from(m: ReplyKeyboardRemove) -> Self {
        ReplyMarkup::Remove(m)
    }
}

impl From<ForceReply> for ReplyMarkup {
    fn from(m: ForceReply) -> Self {
        ReplyMarkup::ForceReply(m)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ButtonStyle, ForceReply, InlineKeyboardButton, InlineKeyboardMarkup, KeyboardButton, ReplyKeyboardMarkup,
        ReplyKeyboardRemove, ReplyMarkup, base,
    };
    use crate::raw::{Deserializable, Serializable, types};

    #[test]
    fn written_markup_reads_back() {
        let inline = InlineKeyboardMarkup::new([
            vec![
                InlineKeyboardButton::callback("a", "data").style(ButtonStyle::Primary),
                InlineKeyboardButton::url("b", "https://t.me"),
            ],
            vec![
                InlineKeyboardButton::switch_inline_query("c", "q"),
                InlineKeyboardButton::switch_inline_query_current_chat("d", ""),
                InlineKeyboardButton::copy_text("e", "copied").style(ButtonStyle::Danger),
            ],
        ]);
        let rows = [
            vec![KeyboardButton::new("phone").request_contact(), KeyboardButton::new("geo").request_location()],
            vec![KeyboardButton::new("plain").style(ButtonStyle::Success)],
        ];
        let keyboard = ReplyKeyboardMarkup::new(rows).one_time().placeholder("pick");
        let markups: [ReplyMarkup; 4] = [
            inline.into(),
            keyboard.into(),
            ReplyKeyboardRemove { selective: true }.into(),
            ForceReply { selective: true, placeholder: Some("why?".into()) }.into(),
        ];
        for markup in markups {
            let raw = base::ReplyMarkup::from_bytes(&markup.write().to_bytes()).unwrap();
            assert_eq!(ReplyMarkup::parse(&raw), Some(markup));
        }
    }

    #[test]
    fn unknown_buttons_hide_the_whole_markup() {
        let button = |r#type| {
            let button = types::KeyboardInlineButton { style: None, text: "x".into(), r#type };
            base::KeyboardInlineButton::KeyboardInlineButton(Box::new(button))
        };
        let markup = |buttons| {
            let row = types::KeyboardInlineButtonRow { buttons };
            let rows = vec![base::KeyboardInlineButtonRow::KeyboardInlineButtonRow(Box::new(row))];
            base::ReplyMarkup::ReplyInlineMarkup(Box::new(types::ReplyInlineMarkup { force_reply: false, rows }))
        };
        let callback = |data: &[u8]| {
            let callback = types::InlineButtonTypeCallback { requires_password: false, data: data.to_vec() };
            button(base::InlineButtonType::InlineButtonTypeCallback(Box::new(callback)))
        };

        assert!(ReplyMarkup::parse(&markup(vec![callback(b"ok")])).is_some());
        let buy = button(base::InlineButtonType::InlineButtonTypeBuy);
        assert_eq!(ReplyMarkup::parse(&markup(vec![callback(b"ok"), buy])), None);
        assert_eq!(ReplyMarkup::parse(&markup(vec![callback(&[0xff])])), None);
    }
}
