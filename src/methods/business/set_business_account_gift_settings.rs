use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::AcceptedGiftTypes;

/// Request built by [`Client::set_business_account_gift_settings`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SetBusinessAccountGiftSettings<'a> {
    client: &'a Client,
    business_connection_id: String,
    show_gift_button: Option<bool>,
    accepted_gift_types: Option<AcceptedGiftTypes>,
}

impl Client {
    /// Changes whether the business account shows a gift button and which gift types it accepts.
    ///
    /// A setting left unset keeps its current value, as does every other global privacy setting of the account.
    /// Setting neither fails with `Error::InvalidArgument`.
    pub fn set_business_account_gift_settings(
        &self,
        business_connection_id: impl Into<String>,
    ) -> SetBusinessAccountGiftSettings<'_> {
        SetBusinessAccountGiftSettings {
            client: self,
            business_connection_id: business_connection_id.into(),
            show_gift_button: None,
            accepted_gift_types: None,
        }
    }
}

impl SetBusinessAccountGiftSettings<'_> {
    /// Shows the gift button in the account's chats when `true` and hides it when `false`.
    pub fn show_gift_button(mut self, show_gift_button: bool) -> Self {
        self.show_gift_button = Some(show_gift_button);
        self
    }

    /// Accepts only these kinds of gifts.
    pub fn accepted_gift_types(mut self, accepted_gift_types: AcceptedGiftTypes) -> Self {
        self.accepted_gift_types = Some(accepted_gift_types);
        self
    }

    async fn send(self) -> Result<()> {
        if self.show_gift_button.is_none() && self.accepted_gift_types.is_none() {
            return Err(Error::InvalidArgument(
                "set_business_account_gift_settings needs show_gift_button or accepted_gift_types",
            ));
        }
        let connection = Some(self.business_connection_id.as_str());
        // The setter replaces every privacy flag, so start from the account's current ones.
        let (base::GlobalPrivacySettings::GlobalPrivacySettings(mut settings), _) =
            self.client.invoke_business(connection, functions::account::GetGlobalPrivacySettings {}).await?;
        if let Some(show_gift_button) = self.show_gift_button {
            settings.display_gifts_button = show_gift_button;
        }
        if let Some(accepted_gift_types) = self.accepted_gift_types {
            settings.disallowed_gifts = Some(accepted_gift_types.write());
        }
        let settings = base::GlobalPrivacySettings::GlobalPrivacySettings(settings);
        self.client.invoke_business(connection, functions::account::SetGlobalPrivacySettings { settings }).await?;
        Ok(())
    }
}

into_future!(SetBusinessAccountGiftSettings, ());
