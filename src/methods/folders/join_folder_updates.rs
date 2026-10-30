use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::PeerRef;

use super::create_folder_invite_link::chatlist;

impl Client {
    pub async fn join_folder_updates(
        &self,
        folder_id: i32,
        chat_ids: impl IntoIterator<Item = impl Into<PeerRef>>,
    ) -> Result<()> {
        let peers = self.resolve_peers(chat_ids).await?;
        if peers.is_empty() {
            return Err(Error::InvalidArgument("join_folder_updates needs at least one chat"));
        }
        self.invoke(&functions::chatlists::JoinChatlistUpdates { chatlist: chatlist(folder_id), peers }).await?;
        Ok(())
    }
}
