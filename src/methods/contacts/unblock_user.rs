use crate::Client;
use crate::enums::BlockList;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

/// Request built by [`Client::unblock_user`].
#[must_use = "a request does nothing until it is awaited"]
pub struct UnblockUser<'a> {
    client: &'a Client,
    user_id: PeerRef,
    list: BlockList,
}

impl Client {
    /// Unblocks a user. Which block list is used depends on [`block_list`](UnblockUser::block_list).
    pub fn unblock_user(&self, user_id: impl Into<PeerRef>) -> UnblockUser<'_> {
        UnblockUser { client: self, user_id: user_id.into(), list: BlockList::Main }
    }
}

impl UnblockUser<'_> {
    /// Chooses the block list. Defaults to [`BlockList::Main`].
    pub fn block_list(mut self, list: BlockList) -> Self {
        self.list = list;
        self
    }

    async fn send(self) -> Result<()> {
        let id = self.client.resolve_peer(self.user_id).await?;
        self.client.invoke(&functions::contacts::Unblock { my_stories_from: self.list.my_stories_from(), id }).await?;
        Ok(())
    }
}

into_future!(UnblockUser, ());
