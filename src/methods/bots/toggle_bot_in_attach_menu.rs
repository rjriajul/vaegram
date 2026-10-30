use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
pub struct ToggleBotInAttachMenu<'a> {
    client: &'a Client,
    bot_id: PeerRef,
    enabled: bool,
    write_allowed: bool,
}

impl Client {
    pub fn toggle_bot_in_attach_menu(&self, bot_id: impl Into<PeerRef>, enabled: bool) -> ToggleBotInAttachMenu<'_> {
        ToggleBotInAttachMenu { client: self, bot_id: bot_id.into(), enabled, write_allowed: false }
    }
}

impl ToggleBotInAttachMenu<'_> {
    pub fn write_allowed(mut self) -> Self {
        self.write_allowed = true;
        self
    }

    async fn send(self) -> Result<bool> {
        let request = functions::messages::ToggleBotInAttachMenu {
            write_allowed: self.write_allowed,
            bot: self.client.resolve_user(&self.bot_id).await?,
            enabled: self.enabled,
        };
        self.client.invoke(&request).await
    }
}

into_future!(ToggleBotInAttachMenu, bool);
