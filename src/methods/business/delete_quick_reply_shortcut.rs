use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn delete_quick_reply_shortcut(&self, shortcut_id: i32) -> Result<bool> {
        self.invoke(&functions::messages::DeleteQuickReplyShortcut { shortcut_id }).await
    }
}
