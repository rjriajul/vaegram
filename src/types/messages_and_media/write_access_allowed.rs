use crate::types::User;

/// A service message saying the user let the bot message them, or was invited to the bot's private chat by its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct WriteAccessAllowed {
    /// The user accepted a write access request from the bot.
    pub from_request: bool,
    /// Short name of the Web App that was launched. `None` if no Web App was launched.
    pub web_app_name: Option<String>,
    /// The user added the bot to the attachment menu.
    pub from_attachment_menu: bool,
    /// The bot's owner, who invited the user to the private chat with the bot through a `t.me/botinvite` link. `None`
    /// for every other way of allowing the bot.
    pub inviter: Option<Box<User>>,
}
