use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Marks your account online, or offline when `offline` is true.
    pub async fn update_status(&self, offline: bool) -> Result<bool> {
        self.invoke(&functions::account::UpdateStatus { offline }).await
    }
}
