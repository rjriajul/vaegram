use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn reorder_usernames<S: Into<String>>(&self, order: impl IntoIterator<Item = S>) -> Result<bool> {
        self.invoke(&functions::account::ReorderUsernames { order: order.into_iter().map(Into::into).collect() }).await
    }
}
