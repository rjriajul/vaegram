use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;

impl Client {
    pub async fn reorder_quick_replies(&self, shortcut_ids: impl IntoIterator<Item = i32>) -> Result<bool> {
        let order: Vec<i32> = shortcut_ids.into_iter().collect();
        if order.is_empty() {
            return Err(Error::InvalidArgument("reorder_quick_replies takes at least one shortcut id"));
        }
        self.invoke(&functions::messages::ReorderQuickReplies { order }).await
    }
}
