use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

impl Client {
    /// Reorders your usernames. `order` lists them, without the `@`, in the order to show them.
    pub async fn reorder_usernames<S: Into<String>>(&self, order: impl IntoIterator<Item = S>) -> Result<()> {
        accepted(
            self.invoke(&functions::account::ReorderUsernames { order: order.into_iter().map(Into::into).collect() })
                .await?,
        )
    }
}
