use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Dialog;

impl Client {
    pub async fn get_dialogs_count(&self, archived: bool) -> Result<i32> {
        let request = functions::messages::GetDialogs {
            exclude_pinned: false,
            folder_id: archived.then_some(1),
            offset_date: 0,
            offset_id: 0,
            offset_peer: base::InputPeer::InputPeerEmpty,
            limit: 1,
            hash: 0,
        };
        let (dialogs, count) = Dialog::unpack(self, self.invoke(&request).await?);
        Ok(count.unwrap_or(dialogs.len() as i32))
    }
}
