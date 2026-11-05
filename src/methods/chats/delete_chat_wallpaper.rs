use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

/// Request built by [`Client::delete_chat_wallpaper`].
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteChatWallpaper<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    restore_previous: bool,
}

impl Client {
    /// Removes the wallpaper you set for a chat.
    pub fn delete_chat_wallpaper(&self, chat_id: impl Into<PeerRef>) -> DeleteChatWallpaper<'_> {
        DeleteChatWallpaper { client: self, chat_id: chat_id.into(), restore_previous: false }
    }
}

impl DeleteChatWallpaper<'_> {
    /// Brings back the previous wallpaper instead of leaving the chat with none.
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
