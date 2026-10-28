use crate::raw::{base, types};
use crate::types::ButtonStyle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplyKeyboardMarkup {
    pub rows: Vec<Vec<KeyboardButton>>,
    pub resize: bool,
    pub one_time: bool,
    pub persistent: bool,
    pub selective: bool,
    pub force_reply: bool,
    pub placeholder: Option<String>,
}

impl ReplyKeyboardMarkup {
    pub fn new(rows: impl IntoIterator<Item = Vec<KeyboardButton>>) -> Self {
        ReplyKeyboardMarkup { rows: rows.into_iter().collect(), resize: true, ..Default::default() }
    }

    pub fn one_time(mut self) -> Self {
        self.one_time = true;
        self
    }

    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = Some(text.into());
        self
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
                let buttons = row.iter().map(KeyboardButton::write).collect();
                base::KeyboardButtonRow::KeyboardButtonRow(Box::new(types::KeyboardButtonRow { buttons }))
            })
            .collect();
        base::ReplyMarkup::ReplyKeyboardMarkup(Box::new(types::ReplyKeyboardMarkup {
            resize: self.resize,
            single_use: self.one_time,
            selective: self.selective,
            persistent: self.persistent,
            force_reply: self.force_reply,
            rows,
            placeholder: self.placeholder.clone(),
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardButton {
    pub text: String,
    pub request_contact: bool,
    pub request_location: bool,
    pub request_managed_bot: Option<KeyboardButtonRequestManagedBot>,
    pub style: ButtonStyle,
}

impl KeyboardButton {
    pub fn new(text: impl Into<String>) -> Self {
        KeyboardButton {
            text: text.into(),
            request_contact: false,
            request_location: false,
            request_managed_bot: None,
            style: ButtonStyle::Default,
        }
    }

    pub fn request_contact(mut self) -> Self {
        self.request_contact = true;
        self
    }

    pub fn request_location(mut self) -> Self {
        self.request_location = true;
        self
    }

    pub fn request_managed_bot(mut self, request: KeyboardButtonRequestManagedBot) -> Self {
        self.request_managed_bot = Some(request);
        self
    }

    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    pub(crate) fn write(&self) -> base::KeyboardButton {
        let kind = match (self.request_contact, self.request_location, &self.request_managed_bot) {
            (true, ..) => base::ButtonType::ButtonTypeRequestPhone,
            (_, true, _) => base::ButtonType::ButtonTypeRequestGeoLocation,
            (.., Some(request)) => request.write(),
            _ => base::ButtonType::ButtonTypeDefault,
        };
        let button = types::KeyboardButton { style: self.style.write(), text: self.text.clone(), r#type: kind };
        base::KeyboardButton::KeyboardButton(Box::new(button))
    }
}

/// Request for the user to create a bot managed by this bot; the result arrives as `ServiceAction::ManagedBotCreated`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyboardButtonRequestManagedBot {
    pub request_id: i32,
    pub suggested_name: Option<String>,
    pub suggested_username: Option<String>,
}

impl KeyboardButtonRequestManagedBot {
    pub fn new(request_id: i32) -> Self {
        KeyboardButtonRequestManagedBot { request_id, ..Default::default() }
    }

    pub fn suggested_name(mut self, name: impl Into<String>) -> Self {
        self.suggested_name = Some(name.into());
        self
    }

    pub fn suggested_username(mut self, username: impl Into<String>) -> Self {
        self.suggested_username = Some(username.into());
        self
    }

    fn write(&self) -> base::ButtonType {
        let peer_type = types::RequestPeerTypeCreateBot {
            bot_managed: true,
            suggested_name: self.suggested_name.clone(),
            suggested_username: self.suggested_username.clone(),
        };
        base::ButtonType::InputButtonTypeRequestPeer(Box::new(types::InputButtonTypeRequestPeer {
            name_requested: false,
            username_requested: false,
            photo_requested: false,
            button_id: self.request_id,
            peer_type: base::RequestPeerType::RequestPeerTypeCreateBot(Box::new(peer_type)),
            max_quantity: 1,
        }))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReplyKeyboardRemove {
    pub selective: bool,
}

impl ReplyKeyboardRemove {
    pub(crate) fn write(&self) -> base::ReplyMarkup {
        base::ReplyMarkup::ReplyKeyboardHide(Box::new(types::ReplyKeyboardHide { selective: self.selective }))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForceReply {
    pub selective: bool,
    pub placeholder: Option<String>,
}

impl ForceReply {
    pub(crate) fn write(&self) -> base::ReplyMarkup {
        base::ReplyMarkup::ReplyKeyboardForceReply(Box::new(types::ReplyKeyboardForceReply {
            single_use: false,
            selective: self.selective,
            placeholder: self.placeholder.clone(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::Serializable;

    fn keyboard(markup: base::ReplyMarkup) -> types::ReplyKeyboardMarkup {
        match markup {
            base::ReplyMarkup::ReplyKeyboardMarkup(m) => *m,
            other => panic!("expected replyKeyboardMarkup, got {other:?}"),
        }
    }

    #[test]
    fn force_reply_maps_to_raw_flag() {
        let plain = ReplyKeyboardMarkup::new([vec![KeyboardButton::new("a")]]);
        assert!(!keyboard(plain.write()).force_reply);
        let raw = plain.force_reply().write();
        assert!(keyboard(raw.clone()).force_reply);
        let bytes = raw.to_bytes();
        assert_eq!(bytes[..4], 0x85dd99d1u32.to_le_bytes());
        assert_eq!(bytes[4..8], 0b10_0001u32.to_le_bytes());
    }

    #[test]
    fn managed_bot_request_becomes_a_create_bot_peer_request() {
        let request = KeyboardButtonRequestManagedBot::new(7).suggested_username("my_bot");
        let base::KeyboardButton::KeyboardButton(button) =
            KeyboardButton::new("x").request_managed_bot(request).write();
        let base::ButtonType::InputButtonTypeRequestPeer(peer) = button.r#type else {
            panic!("expected a peer request");
        };
        let base::RequestPeerType::RequestPeerTypeCreateBot(bot) = peer.peer_type else {
            panic!("expected a create-bot request");
        };
        assert_eq!(
            (peer.button_id, peer.max_quantity, bot.bot_managed, bot.suggested_username.as_deref()),
            (7, 1, true, Some("my_bot"))
        );
    }
}
