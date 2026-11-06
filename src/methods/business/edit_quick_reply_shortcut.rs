use crate::Client;
use crate::errors::{Error, Result};
use crate::methods::accepted;
use crate::raw::functions;

impl Client {
    /// Renames a quick reply shortcut. Fails with `Error::InvalidArgument` if `shortcut` is empty.
    pub async fn edit_quick_reply_shortcut(&self, shortcut_id: i32, shortcut: impl Into<String>) -> Result<()> {
        let shortcut = shortcut.into();
        if shortcut.is_empty() {
            return Err(Error::InvalidArgument("a quick reply shortcut needs a name"));
        }
        accepted(self.invoke(&functions::messages::EditQuickReplyShortcut { shortcut_id, shortcut }).await?)
    }
}
