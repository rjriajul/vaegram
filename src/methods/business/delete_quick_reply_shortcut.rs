use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

impl Client {
    /// Deletes a quick reply shortcut by id.
    pub async fn delete_quick_reply_shortcut(&self, shortcut_id: i32) -> Result<()> {
        accepted(self.invoke(&functions::messages::DeleteQuickReplyShortcut { shortcut_id }).await?)
    }
}
