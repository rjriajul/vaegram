/// A service message saying the user let the bot message them.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WriteAccessAllowed {
    /// The user accepted a write access request from the bot.
    pub from_request: bool,
    /// Short name of the Web App that was launched. `None` if no Web App was launched.
    pub web_app_name: Option<String>,
    /// The user added the bot to the attachment menu.
    pub from_attachment_menu: bool,
}
