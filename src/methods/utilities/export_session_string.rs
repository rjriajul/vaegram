use crate::Client;
use crate::storage::session_string;

impl Client {
    /// The session as one `VAE_…` line, to start from later with `.session_string(…)`.
    /// It is the account's auth key: anyone with it is logged in as you.
    pub fn export_session_string(&self) -> String {
        session_string::export(&self.session_data())
    }
}
