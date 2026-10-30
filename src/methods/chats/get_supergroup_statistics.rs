use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{PeerRef, SupergroupStatistics};

impl Client {
    pub async fn get_supergroup_statistics(&self, chat_id: impl Into<PeerRef>) -> Result<SupergroupStatistics> {
        let channel = self.resolve_channel(&chat_id.into()).await?;
        let stats = self.invoke(&functions::stats::GetMegagroupStats { dark: false, channel }).await?;
        Ok(SupergroupStatistics::parse(self, stats))
    }
}
