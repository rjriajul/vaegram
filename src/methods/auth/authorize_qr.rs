use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::Client;
use crate::errors::Result;
use crate::raw::base;
use crate::types::{LoginToken, User};

impl Client {
    /// Logs in by QR code: `show` gets each fresh `tg://login` link to render until someone scans it.
    /// With two-step verification on this fails with `SESSION_PASSWORD_NEEDED`; call `check_password` then.
    pub async fn authorize_qr(&self, mut show: impl FnMut(&str)) -> Result<User> {
        loop {
            let expires = match self.sign_in_qrcode().await? {
                LoginToken::Authorized(user) => return Ok(user),
                LoginToken::Url { url, expires } => {
                    show(&url);
                    expires
                }
            };
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
            let left = Duration::from_secs((expires as i64 - now).clamp(1, 60) as u64);
            // The lock must go before sign_in_qrcode runs again: a DC switch replaces the receiver.
            let mut updates = self.0.updates.lock().await;
            let _ = tokio::time::timeout(left, async {
                while let Some(incoming) = updates.recv().await {
                    let base::Updates::UpdateShort(u) = incoming.updates else { continue };
                    if matches!(u.update, base::Update::UpdateLoginToken) {
                        return;
                    }
                }
            })
            .await;
        }
    }
}
