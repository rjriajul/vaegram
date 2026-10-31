use crate::Client;
use crate::enums::ProfileTab;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PeerRef;

impl Client {
    pub async fn set_main_profile_tab(&self, chat_id: impl Into<PeerRef>, tab: ProfileTab) -> Result<()> {
        let chat = chat_id.into();
        let tab = tab.write();
        if let (_, base::InputPeer::InputPeerSelf) = self.resolve(&chat).await? {
            self.invoke(&functions::account::SetMainProfileTab { tab }).await?;
        } else {
            let channel = self.resolve_channel(&chat).await?;
            self.invoke(&functions::channels::SetMainProfileTab { channel, tab }).await?;
        }
        Ok(())
    }
}
