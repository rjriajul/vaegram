use crate::raw::{base, types};

/// A link that logs the user in on the bot's website with their Telegram account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginUrl {
    /// The URL opened once the user authorizes the login.
    pub url: String,
    /// Replaces the button's text in forwarded messages.
    pub forward_text: Option<String>,
    /// The bot whose login the user authorizes, when not the sending bot. It has to be linked to the domain.
    /// Only for sending: a login button read from a received message never carries it back.
    pub bot_username: Option<String>,
    /// Asks the user to let the bot write to them.
    pub request_write_access: bool,
}

impl LoginUrl {
    /// Creates a login link to `url` with no extra options.
    pub fn new(url: impl Into<String>) -> Self {
        LoginUrl { url: url.into(), forward_text: None, bot_username: None, request_write_access: false }
    }

    /// Replaces the button's text in forwarded messages.
    pub fn forward_text(mut self, text: impl Into<String>) -> Self {
        self.forward_text = Some(text.into());
        self
    }

    /// Names the bot whose login the user authorizes, instead of the sending bot.
    pub fn bot_username(mut self, username: impl Into<String>) -> Self {
        self.bot_username = Some(username.into());
        self
    }

    /// Asks the user to let the bot write to them.
    pub fn request_write_access(mut self) -> Self {
        self.request_write_access = true;
        self
    }

    pub(crate) fn write(&self, bot: Option<base::InputUser>) -> base::InlineButtonType {
        base::InlineButtonType::InputInlineButtonTypeUrlAuth(Box::new(types::InputInlineButtonTypeUrlAuth {
            request_write_access: self.request_write_access,
            fwd_text: self.forward_text.clone(),
            url: self.url.clone(),
            bot,
        }))
    }
}
