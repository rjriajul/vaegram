use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::{base, functions, types};
use crate::types::PeerRef;

impl Client {
    /// Sets the order of the pinned dialogs of Saved Messages, first to last.
    ///
    /// With `force` set, pinned dialogs that `chat_ids` leaves out are unpinned; without it they keep their pin.
    pub async fn reorder_pinned_saved_dialogs(
        &self,
        chat_ids: impl IntoIterator<Item = impl Into<PeerRef>>,
        force: bool,
    ) -> Result<()> {
        let chat_ids: Vec<PeerRef> = chat_ids.into_iter().map(Into::into).collect();
        let mut order = Vec::with_capacity(chat_ids.len());
        for chat_id in chat_ids {
            let peer = types::InputDialogPeer { peer: self.resolve_peer(chat_id).await? };
            order.push(base::InputDialogPeer::InputDialogPeer(Box::new(peer)));
        }
        accepted(self.invoke(&functions::messages::ReorderPinnedSavedDialogs { force, order }).await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    fn dialog(chat_id: i64) -> base::InputDialogPeer {
        let peer = base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id });
        base::InputDialogPeer::InputDialogPeer(Box::new(types::InputDialogPeer { peer }))
    }

    #[tokio::test]
    async fn sends_the_dialogs_in_order() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes()), Ok(true.to_bytes())]);
        client.reorder_pinned_saved_dialogs([-6_i64, -5], false).await.unwrap();
        client.reorder_pinned_saved_dialogs(vec![-5_i64], true).await.unwrap();
        let first = functions::messages::ReorderPinnedSavedDialogs { force: false, order: vec![dialog(6), dialog(5)] };
        let second = functions::messages::ReorderPinnedSavedDialogs { force: true, order: vec![dialog(5)] };
        assert_eq!(sent.try_recv().unwrap(), first.to_bytes());
        assert_eq!(sent.try_recv().unwrap(), second.to_bytes());
    }
}
