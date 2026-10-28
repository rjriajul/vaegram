use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::storage;

impl Client {
    /// Terminates the authorization on Telegram's side and deletes the local session file.
    pub async fn log_out(&self) -> Result<()> {
        self.invoke(&functions::auth::LogOut {}).await?;
        match &self.0.storage_path {
            Some(path) => Ok(storage::delete(path)?),
            None => Ok(()),
        }
    }
}
