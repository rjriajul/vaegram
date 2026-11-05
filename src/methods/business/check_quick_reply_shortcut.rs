use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;

impl Client {
    /// Returns whether a name is usable for a new quick reply shortcut.
    ///
    /// Fails with `Error::InvalidArgument` if `shortcut` is empty.
    pub async fn check_quick_reply_shortcut(&self, shortcut: impl Into<String>) -> Result<bool> {
        let shortcut = shortcut.into();
        if shortcut.is_empty() {
            return Err(Error::InvalidArgument("a quick reply shortcut needs a name"));
        }
        self.invoke(&functions::messages::CheckQuickReplyShortcut { shortcut }).await
    }
}
