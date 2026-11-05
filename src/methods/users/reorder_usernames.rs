use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Sets the order of your usernames. `order` lists the usernames, without the `@`, in the order to show them.
    pub async fn reorder_usernames<S: Into<String>>(&self, order: impl IntoIterator<Item = S>) -> Result<bool> {
        self.invoke(&functions::account::ReorderUsernames { order: order.into_iter().map(Into::into).collect() }).await
    }
}
