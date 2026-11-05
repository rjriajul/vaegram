use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};

impl Client {
    /// Accepts the terms of service with the given id.
    ///
    /// Take the id from [`TermsOfService`](crate::types::TermsOfService) as returned by
    /// [`get_terms_of_service_update`](Client::get_terms_of_service_update).
    pub async fn accept_terms_of_service(&self, terms_of_service_id: impl Into<String>) -> Result<bool> {
        let id = base::DataJSON::DataJSON(Box::new(types::DataJSON { data: terms_of_service_id.into() }));
        self.invoke(&functions::help::AcceptTermsOfService { id }).await
    }
}
