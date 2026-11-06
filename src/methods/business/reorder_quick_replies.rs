use crate::Client;
use crate::errors::{Error, Result};
use crate::methods::accepted;
use crate::raw::functions;

impl Client {
    /// Sets the order of the quick reply shortcuts to `shortcut_ids`.
    ///
    /// Fails with `Error::InvalidArgument` if `shortcut_ids` is empty.
    pub async fn reorder_quick_replies(&self, shortcut_ids: impl IntoIterator<Item = i32>) -> Result<()> {
        let order: Vec<i32> = shortcut_ids.into_iter().collect();
        if order.is_empty() {
            return Err(Error::InvalidArgument("reorder_quick_replies takes at least one shortcut id"));
        }
        accepted(self.invoke(&functions::messages::ReorderQuickReplies { order }).await?)
    }
}
