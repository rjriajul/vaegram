use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;

impl Client {
    /// Deletes messages from a quick reply shortcut.
    ///
    /// Fails with `Error::InvalidArgument` if `message_ids` is empty.
    pub async fn delete_quick_reply_messages(&self, shortcut_id: i32, message_ids: &[i32]) -> Result<()> {
        if message_ids.is_empty() {
            return Err(Error::InvalidArgument("delete_quick_reply_messages needs at least one message id"));
        }
        let id = message_ids.to_vec();
        self.invoke(&functions::messages::DeleteQuickReplyMessages { shortcut_id, id }).await.map(drop)
    }
}
