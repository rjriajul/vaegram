use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::utils::get_peer_id;

use super::create_folder_invite_link::chatlist;

#[must_use = "a request does nothing until it is awaited"]
pub struct LeaveFolder<'a> {
    client: &'a Client,
    folder_id: i32,
    leave_chats: bool,
}

impl Client {
    pub fn leave_folder(&self, folder_id: i32) -> LeaveFolder<'_> {
        LeaveFolder { client: self, folder_id, leave_chats: false }
    }
}

impl LeaveFolder<'_> {
    pub fn leave_chats(mut self) -> Self {
        self.leave_chats = true;
        self
    }

    async fn send(self) -> Result<()> {
        let client = self.client;
        let peers = if self.leave_chats {
            let request = functions::chatlists::GetLeaveChatlistSuggestions { chatlist: chatlist(self.folder_id) };
            let suggested = client.invoke(&request).await?;
            client.resolve_peers(suggested.iter().map(get_peer_id)).await?
        } else {
            Vec::new()
        };
        client.invoke(&functions::chatlists::LeaveChatlist { chatlist: chatlist(self.folder_id), peers }).await?;
        Ok(())
    }
}

into_future!(LeaveFolder, ());
