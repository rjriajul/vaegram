use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};

impl Client {
    pub async fn accept_terms_of_service(&self, terms_of_service_id: impl Into<String>) -> Result<bool> {
        let id = base::DataJSON::DataJSON(Box::new(types::DataJSON { data: terms_of_service_id.into() }));
        self.invoke(&functions::help::AcceptTermsOfService { id }).await
    }
}
