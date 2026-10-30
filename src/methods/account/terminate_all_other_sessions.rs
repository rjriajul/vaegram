use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn terminate_all_other_sessions(&self) -> Result<bool> {
        self.invoke(&functions::auth::ResetAuthorizations {}).await
    }
}
