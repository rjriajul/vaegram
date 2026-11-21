use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Config;

impl Client {
    /// Returns the parts of Telegram's server configuration a client most often needs: data centers and limits.
    pub async fn get_config(&self) -> Result<Config> {
        let base::Config::Config(config) = self.invoke(&functions::help::GetConfig {}).await?;
        Ok(Config::parse(*config))
    }
}
