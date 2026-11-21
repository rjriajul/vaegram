use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::AppConfig;

impl Client {
    /// Returns Telegram's app configuration: limits and feature switches as JSON, by name.
    ///
    /// The client keeps the last configuration it got and sends its hash, so Telegram answers with the whole
    /// configuration only when it changed.
    pub async fn get_app_config(&self) -> Result<AppConfig> {
        let cached = self.0.app_config.lock().unwrap().clone();
        let hash = cached.as_ref().map_or(0, |c| c.hash);
        match self.invoke(&functions::help::GetAppConfig { hash }).await? {
            base::help::AppConfig::AppConfig(c) => {
                let config = AppConfig::parse(*c);
                *self.0.app_config.lock().unwrap() = Some(config.clone());
                Ok(config)
            }
            base::help::AppConfig::AppConfigNotModified => Ok(cached.unwrap_or_else(AppConfig::empty)),
        }
    }
}
