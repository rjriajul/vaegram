use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{KeyboardButton, PeerRef};

impl Client {
    pub async fn save_prepared_keyboard_button(
        &self,
        user_id: impl Into<PeerRef>,
        button: &KeyboardButton,
    ) -> Result<String> {
        let user_id = self.resolve_user(&user_id.into()).await?;
        let request = functions::bots::RequestWebViewButton { user_id, button: button.write() };
        let base::bots::RequestedButton::RequestedButton(b) = self.invoke(&request).await?;
        Ok(b.webapp_req_id)
    }
}
