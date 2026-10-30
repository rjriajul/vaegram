use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteChatWallpaper<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    restore_previous: bool,
}

impl Client {
    pub fn delete_chat_wallpaper(&self, chat_id: impl Into<PeerRef>) -> DeleteChatWallpaper<'_> {
        DeleteChatWallpaper { client: self, chat_id: chat_id.into(), restore_previous: false }
    }
}

impl DeleteChatWallpaper<'_> {
    pub fn restore_previous(mut self) -> Self {
        self.restore_previous = true;
        self
    }

    async fn send(self) -> Result<()> {
        let request = functions::messages::SetChatWallPaper {
            for_both: false,
            revert: self.restore_previous,
            peer: self.client.resolve_peer(self.chat_id).await?,
            wallpaper: None,
            settings: None,
            id: None,
        };
        self.client.invoke(&request).await?;
        Ok(())
    }
}

into_future!(DeleteChatWallpaper, ());
