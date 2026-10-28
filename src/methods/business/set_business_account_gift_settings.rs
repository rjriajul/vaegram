use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::AcceptedGiftTypes;

impl Client {
    pub async fn set_business_account_gift_settings(
        &self,
        business_connection_id: &str,
        show_gift_button: bool,
        accepted_gift_types: AcceptedGiftTypes,
    ) -> Result<()> {
        // The setter replaces every privacy flag, so start from the account's current ones.
        let (base::GlobalPrivacySettings::GlobalPrivacySettings(mut settings), _) =
            self.invoke_business(Some(business_connection_id), functions::account::GetGlobalPrivacySettings {}).await?;
        settings.display_gifts_button = show_gift_button;
        settings.disallowed_gifts = Some(accepted_gift_types.write());
        let settings = base::GlobalPrivacySettings::GlobalPrivacySettings(settings);
        self.invoke_business(Some(business_connection_id), functions::account::SetGlobalPrivacySettings { settings })
            .await?;
        Ok(())
    }
}
