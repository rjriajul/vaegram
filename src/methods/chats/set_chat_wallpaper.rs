use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{ChatBackground, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SetChatWallpaper<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    wallpaper: base::InputWallPaper,
    settings: base::WallPaperSettings,
    for_both: bool,
}

impl Client {
    pub fn set_chat_wallpaper(&self, chat_id: impl Into<PeerRef>, wallpaper: &ChatBackground) -> SetChatWallpaper<'_> {
        let (settings, wallpaper) = (wallpaper.write_settings(), wallpaper.write());
        SetChatWallpaper { client: self, chat_id: chat_id.into(), wallpaper, settings, for_both: false }
    }
}

impl SetChatWallpaper<'_> {
    pub fn for_both(mut self) -> Self {
        self.for_both = true;
        self
    }

    async fn send(self) -> Result<()> {
        let request = functions::messages::SetChatWallPaper {
            for_both: self.for_both,
            revert: false,
            peer: self.client.resolve_peer(self.chat_id).await?,
            wallpaper: Some(self.wallpaper),
            settings: Some(self.settings),
            id: None,
        };
        self.client.invoke(&request).await?;
        Ok(())
    }
}

into_future!(SetChatWallpaper, ());
