use crate::raw::{base, types};
use crate::types::{
    ButtonStyle, KeyboardButtonPollType, KeyboardButtonRequestChat, KeyboardButtonRequestUsers, WebAppInfo,
};

/// A custom keyboard that replaces the user's regular keyboard.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplyKeyboardMarkup {
    /// Buttons, one inner `Vec` per row.
    pub rows: Vec<Vec<KeyboardButton>>,
    /// Fits the keyboard to its buttons instead of the default height.
    /// [`new`](ReplyKeyboardMarkup::new) turns it on.
    pub resize: bool,
    /// Hides the keyboard after one button press.
    pub one_time: bool,
    /// Keeps the keyboard shown when the regular keyboard is hidden.
    pub persistent: bool,
    /// Shows the keyboard only to some users in a group.
    pub selective: bool,
    /// Also forces the user to reply, as [`ForceReply`] does.
    pub force_reply: bool,
    /// Hint shown in the input field while the keyboard is up.
    pub placeholder: Option<String>,
}

impl ReplyKeyboardMarkup {
    /// Creates a keyboard from rows of buttons, with `resize` on.
    pub fn new(rows: impl IntoIterator<Item = Vec<KeyboardButton>>) -> Self {
        ReplyKeyboardMarkup { rows: rows.into_iter().collect(), resize: true, ..Default::default() }
    }

    /// Sets whether the keyboard hides after one button press; `true` hides it.
    #[must_use]
    pub fn one_time(mut self, one_time: bool) -> Self {
        self.one_time = one_time;
        self
    }

    /// Shows `text` in the input field while the keyboard is up.
    #[must_use]
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Sets whether the keyboard also forces the user to reply, as [`ForceReply`] does; `true` forces it.
    #[must_use]
    pub fn force_reply(mut self, force_reply: bool) -> Self {
        self.force_reply = force_reply;
        self
    }

    /// Sets whether the keyboard fits its buttons instead of taking the default height; `new` turns it on.
    #[must_use]
    pub fn resize(mut self, resize: bool) -> Self {
        self.resize = resize;
        self
    }

    /// Sets whether the keyboard stays shown when the regular keyboard is hidden.
    #[must_use]
    pub fn persistent(mut self, persistent: bool) -> Self {
        self.persistent = persistent;
        self
    }

    /// Sets whether only some users in a group see the keyboard.
    #[must_use]
    pub fn selective(mut self, selective: bool) -> Self {
        self.selective = selective;
        self
    }

    pub(crate) fn parse(raw: &types::ReplyKeyboardMarkup, rows: Vec<Vec<KeyboardButton>>) -> Self {
        ReplyKeyboardMarkup {
            rows,
            resize: raw.resize,
            one_time: raw.single_use,
            persistent: raw.persistent,
            selective: raw.selective,
            force_reply: raw.force_reply,
            placeholder: raw.placeholder.clone(),
        }
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

/// A reply keyboard button. When several request fields are set, the first in this order is the one sent:
/// `request_contact`, `request_location`, `request_managed_bot`, `request_users`, `request_chat`, `request_poll`,
/// `web_app`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardButton {
    /// Label of the button. Sent as a plain message when pressed, unless a request field is set.
    pub text: String,
    /// Sends the user's phone number when pressed.
    pub request_contact: bool,
    /// Sends the user's current location when pressed.
    pub request_location: bool,
    /// Asks the user to create a bot managed by this bot.
    pub request_managed_bot: Option<KeyboardButtonRequestManagedBot>,
    /// Asks the user to pick users to share with the bot.
    pub request_users: Option<KeyboardButtonRequestUsers>,
    /// Asks the user to pick a chat to share with the bot.
    pub request_chat: Option<KeyboardButtonRequestChat>,
    /// Asks the user to create a poll of the given type.
    pub request_poll: Option<KeyboardButtonPollType>,
    /// Opens a mini app.
    pub web_app: Option<WebAppInfo>,
    /// Color of the button.
    pub style: ButtonStyle,
    /// A custom emoji shown before the text.
    pub icon_custom_emoji_id: Option<i64>,
}

impl KeyboardButton {
    /// Creates a plain text button.
    pub fn new(text: impl Into<String>) -> Self {
        KeyboardButton {
            text: text.into(),
            request_contact: false,
            request_location: false,
            request_managed_bot: None,
            request_users: None,
            request_chat: None,
            request_poll: None,
            web_app: None,
            style: ButtonStyle::Default,
            icon_custom_emoji_id: None,
        }
    }

    /// Sets whether the button sends the user's phone number when pressed; `true` sends it.
    pub fn request_contact(mut self, request_contact: bool) -> Self {
        self.request_contact = request_contact;
        self
    }

    /// Sets whether the button sends the user's current location when pressed; `true` sends it.
    pub fn request_location(mut self, request_location: bool) -> Self {
        self.request_location = request_location;
        self
    }

    /// Makes the button ask the user to create a managed bot.
    pub fn request_managed_bot(mut self, request: KeyboardButtonRequestManagedBot) -> Self {
        self.request_managed_bot = Some(request);
        self
    }

    /// Makes the button ask the user to pick users to share.
    pub fn request_users(mut self, request: KeyboardButtonRequestUsers) -> Self {
        self.request_users = Some(request);
        self
    }

    /// Makes the button ask the user to pick a chat to share.
    pub fn request_chat(mut self, request: KeyboardButtonRequestChat) -> Self {
        self.request_chat = Some(request);
        self
    }

    /// Makes the button ask the user to create a poll.
    pub fn request_poll(mut self, request: KeyboardButtonPollType) -> Self {
        self.request_poll = Some(request);
        self
    }

    /// Makes the button open a mini app.
    pub fn web_app(mut self, web_app: WebAppInfo) -> Self {
        self.web_app = Some(web_app);
        self
    }

    /// Colors the button.
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    /// Shows the custom emoji `id` before the text.
    pub fn icon_custom_emoji_id(mut self, id: i64) -> Self {
        self.icon_custom_emoji_id = Some(id);
        self
    }

    pub(crate) fn write(&self) -> base::KeyboardButton {
        let kind = if self.request_contact {
            base::ButtonType::ButtonTypeRequestPhone
        } else if self.request_location {
            base::ButtonType::ButtonTypeRequestGeoLocation
        } else if let Some(request) = &self.request_managed_bot {
            request.write()
        } else if let Some(request) = &self.request_users {
            request.write()
        } else if let Some(request) = &self.request_chat {
            request.write()
        } else if let Some(request) = &self.request_poll {
            request.write()
        } else if let Some(web_app) = &self.web_app {
            base::ButtonType::ButtonTypeSimpleWebView(Box::new(types::ButtonTypeSimpleWebView {
                url: web_app.url.clone(),
            }))
        } else {
            base::ButtonType::ButtonTypeDefault
        };
        let style = self.style.write_with_icon(self.icon_custom_emoji_id);
        let button = types::KeyboardButton { style, text: self.text.clone(), r#type: kind };
        base::KeyboardButton::KeyboardButton(Box::new(button))
    }
}

/// Request for the user to create a bot managed by this bot. The result arrives as
/// `ServiceAction::ManagedBotCreated`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyboardButtonRequestManagedBot {
    /// Id echoed back in the result, to tell requests apart.
    pub request_id: i32,
    /// Name proposed for the new bot. `None` leaves the choice to the user.
    pub suggested_name: Option<String>,
    /// Username proposed for the new bot. `None` leaves the choice to the user.
    pub suggested_username: Option<String>,
}

impl KeyboardButtonRequestManagedBot {
    /// Creates a request with the given id and no suggestions.
    pub fn new(request_id: i32) -> Self {
        KeyboardButtonRequestManagedBot { request_id, ..Default::default() }
    }

    /// Proposes a name for the new bot.
    pub fn suggested_name(mut self, name: impl Into<String>) -> Self {
        self.suggested_name = Some(name.into());
        self
    }

    /// Proposes a username for the new bot.
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

/// Removes the custom keyboard and brings back the regular one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReplyKeyboardRemove {
    /// Removes the keyboard only for some users in a group.
    pub selective: bool,
}

impl ReplyKeyboardRemove {
    /// Removes the keyboard for everyone, like `Default`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether the keyboard is removed only for some users in a group.
    #[must_use]
    pub fn selective(mut self, selective: bool) -> Self {
        self.selective = selective;
        self
    }

    pub(crate) fn write(&self) -> base::ReplyMarkup {
        base::ReplyMarkup::ReplyKeyboardHide(types::ReplyKeyboardHide { selective: self.selective })
    }
}

/// Makes the user's client open a reply to the message.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForceReply {
    /// Applies only to some users in a group.
    pub selective: bool,
    /// Hint shown in the input field while the reply is pending.
    pub placeholder: Option<String>,
}

impl ForceReply {
    /// Applies to everyone and shows no hint, like `Default`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether only some users in a group are asked to reply.
    #[must_use]
    pub fn selective(mut self, selective: bool) -> Self {
        self.selective = selective;
        self
    }

    /// Shows `placeholder` in the input field while the reply is pending.
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub(crate) fn parse(raw: &types::ReplyKeyboardForceReply) -> Self {
        ForceReply { selective: raw.selective, placeholder: raw.placeholder.clone() }
    }

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
        let raw = plain.force_reply(true).write();
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

    #[test]
    fn keyboard_setters_override_new() {
        let markup = ReplyKeyboardMarkup::new([vec![KeyboardButton::new("a")]]);
        let plain = keyboard(markup.write());
        assert!(plain.resize && !plain.persistent && !plain.selective && !plain.single_use);
        let set = keyboard(markup.resize(false).persistent(true).selective(true).write());
        assert!(!set.resize && set.persistent && set.selective && !set.single_use);
    }

    #[test]
    fn bool_setters_round_trip_their_fields() {
        let markup = ReplyKeyboardMarkup::new([vec![KeyboardButton::new("a")]]);
        let on = markup.clone().one_time(true).force_reply(true);
        assert!(on.one_time && on.force_reply);
        let off = on.one_time(false).force_reply(false);
        assert_eq!(off, markup);
        let button = KeyboardButton::new("b").request_contact(true).request_location(true);
        assert!(button.request_contact && button.request_location);
        let button = button.request_contact(false).request_location(false);
        assert_eq!(button, KeyboardButton::new("b"));
    }

    #[test]
    fn remove_and_force_reply_build_without_a_literal() {
        let base::ReplyMarkup::ReplyKeyboardHide(hide) = ReplyKeyboardRemove::new().selective(true).write() else {
            panic!("expected replyKeyboardHide");
        };
        assert!(hide.selective);
        let base::ReplyMarkup::ReplyKeyboardForceReply(reply) = ForceReply::new().placeholder("why?").write() else {
            panic!("expected replyKeyboardForceReply");
        };
        assert_eq!((reply.selective, reply.placeholder.as_deref()), (false, Some("why?")));
    }
}
