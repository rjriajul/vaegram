use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::PeerRef;

impl Client {
    pub async fn archive_chats(&self, chat_ids: impl IntoIterator<Item = impl Into<PeerRef>>) -> Result<()> {
        self.move_to_folder(chat_ids.into_iter().map(Into::into).collect(), 1).await
    }

    pub(crate) async fn move_to_folder(&self, chat_ids: Vec<PeerRef>, folder_id: i32) -> Result<()> {
        if chat_ids.is_empty() {
            return Err(Error::InvalidArgument("archiving and unarchiving take at least one chat"));
        }
        let mut folder_peers = Vec::with_capacity(chat_ids.len());
        for chat_id in chat_ids {
            let peer = types::InputFolderPeer { peer: self.resolve_peer(chat_id).await?, folder_id };
            folder_peers.push(base::InputFolderPeer::InputFolderPeer(Box::new(peer)));
        }
        self.invoke(&functions::folders::EditPeerFolders { folder_peers }).await?;
        Ok(())
    }
}
