use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    /// Moves chats from the archive back to the main chat list.
    ///
    /// Fails with `Error::InvalidArgument` if `chat_ids` is empty.
    pub async fn unarchive_chats(&self, chat_ids: impl IntoIterator<Item = impl Into<PeerRef>>) -> Result<()> {
        self.move_to_folder(chat_ids.into_iter().map(Into::into).collect(), 0).await
    }
}
