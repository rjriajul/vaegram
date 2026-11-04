use crate::Client;
use crate::enums::BlockList;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
pub struct BlockUser<'a> {
    client: &'a Client,
    user_id: PeerRef,
    list: BlockList,
}

impl Client {
    pub fn block_user(&self, user_id: impl Into<PeerRef>) -> BlockUser<'_> {
        BlockUser { client: self, user_id: user_id.into(), list: BlockList::Main }
    }
}

impl BlockUser<'_> {
    /// Chooses the block list; the default is the main one.
    pub fn block_list(mut self, list: BlockList) -> Self {
        self.list = list;
        self
    }

    async fn send(self) -> Result<()> {
        let id = self.client.resolve_peer(self.user_id).await?;
        self.client.invoke(&functions::contacts::Block { my_stories_from: self.list.my_stories_from(), id }).await?;
        Ok(())
    }
}

into_future!(BlockUser, ());
