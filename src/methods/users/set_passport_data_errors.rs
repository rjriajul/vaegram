use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{PassportElementError, PeerRef};

impl Client {
    /// Reports problems in a user's Telegram Passport data, which the user must fix before resubmitting.
    pub async fn set_passport_data_errors(
        &self,
        user_id: impl Into<PeerRef>,
        errors: impl IntoIterator<Item = PassportElementError>,
    ) -> Result<()> {
        let request = functions::users::SetSecureValueErrors {
            id: self.resolve_user(&user_id.into()).await?,
            errors: errors.into_iter().map(PassportElementError::write).collect(),
        };
        self.invoke(&request).await?;
        Ok(())
    }
}
