use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Json;

impl Client {
    pub async fn get_app_config(&self) -> Result<Json> {
        match self.invoke(&functions::help::GetAppConfig { hash: 0 }).await? {
            base::help::AppConfig::AppConfig(c) => Ok(Json::parse(c.config)),
            base::help::AppConfig::AppConfigNotModified => Ok(Json::Object(Vec::new())),
        }
    }
}
