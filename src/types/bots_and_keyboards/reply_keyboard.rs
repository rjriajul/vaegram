use crate::raw::{base, types};
use crate::types::ButtonStyle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplyKeyboardMarkup {
    pub rows: Vec<Vec<KeyboardButton>>,
    pub resize: bool,
    pub one_time: bool,
    pub persistent: bool,
    pub selective: bool,
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
            force_reply: false,
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
    pub style: ButtonStyle,
}

impl KeyboardButton {
    pub fn new(text: impl Into<String>) -> Self {
        KeyboardButton {
            text: text.into(),
            request_contact: false,
            request_location: false,
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

    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    fn write(&self) -> base::KeyboardButton {
        let kind = match (self.request_contact, self.request_location) {
            (true, _) => base::ButtonType::ButtonTypeRequestPhone,
            (_, true) => base::ButtonType::ButtonTypeRequestGeoLocation,
            _ => base::ButtonType::ButtonTypeDefault,
        };
        let button = types::KeyboardButton { style: self.style.write(), text: self.text.clone(), r#type: kind };
        base::KeyboardButton::KeyboardButton(Box::new(button))
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
