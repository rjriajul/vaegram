use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::GlobalPrivacySettings;

impl Client {
    /// Replaces the account-wide privacy settings and returns the ones Telegram stored.
    pub async fn set_global_privacy_settings(&self, settings: GlobalPrivacySettings) -> Result<GlobalPrivacySettings> {
        let request = functions::account::SetGlobalPrivacySettings { settings: settings.write() };
        Ok(GlobalPrivacySettings::parse(self.invoke(&request).await?))
    }
}
