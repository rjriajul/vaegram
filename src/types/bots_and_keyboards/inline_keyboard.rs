use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::types::{LoginUrl, PeerRef, SwitchInlineQueryChosenChat, WebAppInfo};

/// A keyboard of buttons attached to a message.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct InlineKeyboardMarkup {
    /// The buttons, one inner `Vec` per row.
    pub rows: Vec<Vec<InlineKeyboardButton>>,
    /// Also forces the user to reply, as [`ForceReply`](crate::types::ForceReply) does.
    pub force_reply: bool,
}

impl InlineKeyboardMarkup {
    /// Creates a keyboard from rows of buttons.
    pub fn new(rows: impl IntoIterator<Item = Vec<InlineKeyboardButton>>) -> Self {
        InlineKeyboardMarkup { rows: rows.into_iter().collect(), force_reply: false }
    }

    /// Sets whether the keyboard also forces the user to reply, as [`ForceReply`](crate::types::ForceReply) does;
    /// `true` forces it.
    #[must_use]
    pub fn force_reply(mut self, force_reply: bool) -> Self {
        self.force_reply = force_reply;
        self
    }

    /// Resolves the users that profile and login buttons name. Each user not known yet costs a request.
    pub(crate) async fn write(&self, client: &Client) -> Result<base::ReplyMarkup> {
        let mut rows = Vec::with_capacity(self.rows.len());
        for row in &self.rows {
            let mut buttons = Vec::with_capacity(row.len());
            for button in row {
                buttons.push(button.write(client).await?);
            }
            rows.push(base::KeyboardInlineButtonRow::KeyboardInlineButtonRow(Box::new(
                types::KeyboardInlineButtonRow { buttons },
            )));
        }
        Ok(base::ReplyMarkup::ReplyInlineMarkup(Box::new(types::ReplyInlineMarkup {
            force_reply: self.force_reply,
            rows,
        })))
    }

    pub(crate) async fn write_option(
        markup: Option<&InlineKeyboardMarkup>,
        client: &Client,
    ) -> Result<Option<base::ReplyMarkup>> {
        match markup {
            Some(markup) => Ok(Some(markup.write(client).await?)),
            None => Ok(None),
        }
    }
}

/// Color of a keyboard button.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ButtonStyle {
    /// The client's normal button color.
    #[default]
    Default,
    /// Highlighted as the main action.
    Primary,
    /// Marks a destructive action.
    Danger,
    /// Marks a confirming action.
    Success,
}

impl ButtonStyle {
    /// The style, carrying a custom emoji icon when there is one (a plain button with an icon still needs it).
    pub(crate) fn write_with_icon(self, icon: Option<i64>) -> Option<base::KeyboardButtonStyle> {
        let (bg_primary, bg_danger, bg_success) = match self {
            ButtonStyle::Default if icon.is_none() => return None,
            ButtonStyle::Default => (false, false, false),
            ButtonStyle::Primary => (true, false, false),
            ButtonStyle::Danger => (false, true, false),
            ButtonStyle::Success => (false, false, true),
        };
        let style = types::KeyboardButtonStyle { bg_primary, bg_danger, bg_success, icon };
        Some(base::KeyboardButtonStyle::KeyboardButtonStyle(Box::new(style)))
    }
}

/// A button on an [`InlineKeyboardMarkup`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct InlineKeyboardButton {
    /// The label.
    pub text: String,
    /// What pressing the button does.
    pub action: InlineAction,
    /// The button's color.
    pub style: ButtonStyle,
    /// A custom emoji shown before the text.
    pub icon_custom_emoji_id: Option<i64>,
}

/// What an [`InlineKeyboardButton`] does when pressed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InlineAction {
    /// Sends the data to the bot as a [`CallbackQuery`](crate::types::CallbackQuery).
    Callback(String),
    /// Opens a link.
    Url(String),
    /// Lets the user pick a chat and starts an inline query with this text there.
    SwitchInlineQuery(String),
    /// Starts an inline query with this text in the current chat.
    SwitchInlineQueryCurrentChat(String),
    /// Copies the text to the clipboard.
    CopyText(String),
    /// A button that does nothing when pressed.
    Disabled,
    /// Opens the game of a game message.
    CallbackGame,
    /// Opens a mini app.
    WebApp(WebAppInfo),
    /// Logs the user in on a website.
    LoginUrl(LoginUrl),
    /// Opens the profile of the user with this id. Sending the keyboard fails if the user can't be resolved.
    UserProfile(i64),
    /// Opens a chat picker and starts an inline query there.
    SwitchInlineQueryChosenChat(SwitchInlineQueryChosenChat),
    /// Pays an invoice. It has to be the first button of the first row.
    Pay,
    /// Like `Callback`, but the user has to enter their cloud password first.
    CallbackWithPassword(String),
}

impl InlineKeyboardButton {
    fn new(text: impl Into<String>, action: InlineAction) -> Self {
        InlineKeyboardButton { text: text.into(), action, style: ButtonStyle::Default, icon_custom_emoji_id: None }
    }

    /// A button that sends `data` to the bot when pressed.
    pub fn callback(text: impl Into<String>, data: impl Into<String>) -> Self {
        Self::new(text, InlineAction::Callback(data.into()))
    }

    /// A button that opens a link.
    pub fn url(text: impl Into<String>, url: impl Into<String>) -> Self {
        Self::new(text, InlineAction::Url(url.into()))
    }

    /// A button that starts an inline query with `query` in a chat the user picks.
    pub fn switch_inline_query(text: impl Into<String>, query: impl Into<String>) -> Self {
        Self::new(text, InlineAction::SwitchInlineQuery(query.into()))
    }

    /// A button that starts an inline query with `query` in the current chat.
    pub fn switch_inline_query_current_chat(text: impl Into<String>, query: impl Into<String>) -> Self {
        Self::new(text, InlineAction::SwitchInlineQueryCurrentChat(query.into()))
    }

    /// A button that copies `copy` to the clipboard.
    pub fn copy_text(text: impl Into<String>, copy: impl Into<String>) -> Self {
        Self::new(text, InlineAction::CopyText(copy.into()))
    }

    /// A button that does nothing when pressed.
    pub fn disabled(text: impl Into<String>) -> Self {
        Self::new(text, InlineAction::Disabled)
    }

    /// Opens the game of a `send_game` message. It has to be the first button of the first row.
    pub fn callback_game(text: impl Into<String>) -> Self {
        Self::new(text, InlineAction::CallbackGame)
    }

    /// A button that opens a mini app.
    pub fn web_app(text: impl Into<String>, web_app: WebAppInfo) -> Self {
        Self::new(text, InlineAction::WebApp(web_app))
    }

    /// A button that logs the user in on a website.
    pub fn login_url(text: impl Into<String>, login_url: LoginUrl) -> Self {
        Self::new(text, InlineAction::LoginUrl(login_url))
    }

    /// A button that opens a user's profile. Sending the keyboard fails if the user can't be resolved.
    pub fn user_profile(text: impl Into<String>, user_id: i64) -> Self {
        Self::new(text, InlineAction::UserProfile(user_id))
    }

    /// A button that opens a chat picker and starts an inline query there.
    pub fn switch_inline_query_chosen_chat(text: impl Into<String>, chosen: SwitchInlineQueryChosenChat) -> Self {
        Self::new(text, InlineAction::SwitchInlineQueryChosenChat(chosen))
    }

    /// A button that pays an invoice. It has to be the first button of the first row.
    pub fn pay(text: impl Into<String>) -> Self {
        Self::new(text, InlineAction::Pay)
    }

    /// Like [`callback`](Self::callback), but the user has to enter their cloud password first.
    pub fn callback_with_password(text: impl Into<String>, data: impl Into<String>) -> Self {
        Self::new(text, InlineAction::CallbackWithPassword(data.into()))
    }

    /// Changes the button's color.
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    /// Shows the custom emoji `id` before the text, as the Bot API's `icon_custom_emoji_id`.
    pub fn icon_custom_emoji_id(mut self, id: i64) -> Self {
        self.icon_custom_emoji_id = Some(id);
        self
    }

    /// Looks up the user a profile or login button names.
    pub(crate) async fn write(&self, client: &Client) -> Result<base::KeyboardInlineButton> {
        let user = match &self.action {
            InlineAction::UserProfile(id) => Some(client.resolve_user(&PeerRef::from(*id)).await?),
            InlineAction::LoginUrl(LoginUrl { bot_username: Some(bot), .. }) => {
                Some(client.resolve_user(&PeerRef::from(bot.as_str())).await?)
            }
            _ => None,
        };
        self.write_with(user)
    }

    /// Without a client, so a profile button or a login button naming a bot is refused.
    pub(crate) fn write_offline(&self) -> Result<base::KeyboardInlineButton> {
        self.write_with(None)
    }

    fn write_with(&self, user: Option<base::InputUser>) -> Result<base::KeyboardInlineButton> {
        use base::InlineButtonType as T;
        let unresolved = Error::InvalidArgument("a button naming a user can't be written without looking the user up");
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
            InlineAction::CallbackWithPassword(data) => {
                T::InlineButtonTypeCallback(Box::new(types::InlineButtonTypeCallback {
                    requires_password: true,
                    data: data.as_bytes().to_vec(),
                }))
            }
            InlineAction::Url(url) => T::InlineButtonTypeUrl(Box::new(types::InlineButtonTypeUrl { url: url.clone() })),
            InlineAction::SwitchInlineQuery(q) => switch(q, false),
            InlineAction::SwitchInlineQueryCurrentChat(q) => switch(q, true),
            InlineAction::CopyText(copy) => {
                T::InlineButtonTypeCopy(Box::new(types::InlineButtonTypeCopy { copy_text: copy.clone() }))
            }
            InlineAction::Disabled => T::InlineButtonTypeDisabled,
            InlineAction::CallbackGame => T::InlineButtonTypeGame,
            InlineAction::WebApp(app) => {
                T::InlineButtonTypeWebView(Box::new(types::InlineButtonTypeWebView { url: app.url.clone() }))
            }
            InlineAction::LoginUrl(login) if login.bot_username.is_some() && user.is_none() => return Err(unresolved),
            InlineAction::LoginUrl(login) => login.write(user),
            InlineAction::UserProfile(_) => {
                let user_id = user.ok_or(unresolved)?;
                T::InputInlineButtonTypeUserProfile(Box::new(types::InputInlineButtonTypeUserProfile { user_id }))
            }
            InlineAction::SwitchInlineQueryChosenChat(chosen) => chosen.write(),
            InlineAction::Pay => T::InlineButtonTypeBuy,
        };
        let style = self.style.write_with_icon(self.icon_custom_emoji_id);
        let button = types::KeyboardInlineButton { style, text: self.text.clone(), r#type: kind };
        Ok(base::KeyboardInlineButton::KeyboardInlineButton(Box::new(button)))
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

    #[tokio::test]
    async fn force_reply_maps_to_raw_flag() {
        let client = Client::offline();
        let plain = InlineKeyboardMarkup::new([vec![InlineKeyboardButton::callback("a", "b")]]);
        assert!(!inline(plain.write(&client).await.unwrap()).force_reply);
        let raw = plain.force_reply(true).write(&client).await.unwrap();
        assert!(inline(raw.clone()).force_reply);
        let bytes = raw.to_bytes();
        assert_eq!(bytes[..4], 0xb2b15770u32.to_le_bytes());
        assert_eq!(bytes[4..8], (1u32 << 5).to_le_bytes());
    }

    #[test]
    fn force_reply_setter_round_trips_the_field() {
        let markup = InlineKeyboardMarkup::new([vec![InlineKeyboardButton::callback("a", "b")]]);
        assert!(markup.clone().force_reply(true).force_reply);
        assert!(!markup.force_reply(true).force_reply(false).force_reply);
    }

    #[test]
    fn icons_ride_in_the_style_even_without_a_color() {
        let style = |b: InlineKeyboardButton| {
            let base::KeyboardInlineButton::KeyboardInlineButton(button) = b.write_offline().unwrap();
            button.style.map(|base::KeyboardButtonStyle::KeyboardButtonStyle(s)| *s)
        };
        assert_eq!(style(InlineKeyboardButton::url("a", "https://t.me")), None);
        let plain = style(InlineKeyboardButton::url("a", "https://t.me").icon_custom_emoji_id(7)).unwrap();
        assert_eq!((plain.icon, plain.bg_primary), (Some(7), false));
        let primary =
            InlineKeyboardButton::url("a", "https://t.me").style(ButtonStyle::Primary).icon_custom_emoji_id(9);
        let primary = style(primary).unwrap();
        assert_eq!((primary.icon, primary.bg_primary), (Some(9), true));
    }

    #[tokio::test]
    async fn disabled_button_uses_disabled_type() {
        let markup = InlineKeyboardMarkup::new([vec![InlineKeyboardButton::disabled("off")]]);
        let rows = inline(markup.write(&Client::offline()).await.unwrap()).rows;
        let base::KeyboardInlineButtonRow::KeyboardInlineButtonRow(row) = &rows[0];
        let base::KeyboardInlineButton::KeyboardInlineButton(button) = &row.buttons[0];
        assert_eq!(button.text, "off");
        assert_eq!(button.r#type, base::InlineButtonType::InlineButtonTypeDisabled);
        assert_eq!(row.buttons[0].to_bytes()[..4], 0x11c1a322u32.to_le_bytes());
        assert_eq!(button.r#type.to_bytes(), 0xa438619du32.to_le_bytes());
    }
}
