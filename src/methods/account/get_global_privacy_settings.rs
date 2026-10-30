use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::GlobalPrivacySettings;

impl Client {
    pub async fn get_global_privacy_settings(&self) -> Result<GlobalPrivacySettings> {
        let settings = self.invoke(&functions::account::GetGlobalPrivacySettings {}).await?;
        Ok(GlobalPrivacySettings::parse(settings))
    }
}
