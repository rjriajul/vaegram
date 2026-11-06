use crate::Client;
use crate::errors::Result;
use crate::types::ActiveSession;

impl ActiveSession {
    /// Ends this session.
    pub async fn reset(&self, client: &Client) -> Result<()> {
        client.terminate_session(self.id).await
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, functions};
    use crate::types::ActiveSession;

    #[tokio::test]
    async fn reset_ends_this_session() {
        let session = crate::raw::types::Authorization {
            current: false,
            official_app: false,
            password_pending: false,
            encrypted_requests_disabled: false,
            call_requests_disabled: false,
            unconfirmed: false,
            hash: -77,
            device_model: String::new(),
            platform: String::new(),
            system_version: String::new(),
            api_id: 0,
            app_name: String::new(),
            app_version: String::new(),
            date_created: 0,
            date_active: 0,
            ip: String::new(),
            country: String::new(),
            region: String::new(),
        };
        let (client, mut rx) = Client::recording();
        let _ = ActiveSession::parse(session).reset(&client).await;
        assert_eq!(rx.try_recv().unwrap(), functions::account::ResetAuthorization { hash: -77 }.to_bytes());
    }
}
