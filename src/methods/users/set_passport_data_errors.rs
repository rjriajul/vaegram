use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{PassportElementError, PeerRef};

impl Client {
    pub async fn set_passport_data_errors(
        &self,
        user_id: impl Into<PeerRef>,
        errors: impl IntoIterator<Item = PassportElementError>,
    ) -> Result<bool> {
        let request = functions::users::SetSecureValueErrors {
            id: self.resolve_user(&user_id.into()).await?,
            errors: errors.into_iter().map(PassportElementError::write).collect(),
        };
        self.invoke(&request).await
    }
}
