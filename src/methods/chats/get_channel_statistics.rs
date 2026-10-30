use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{ChannelStatistics, PeerRef};

impl Client {
    pub async fn get_channel_statistics(&self, chat_id: impl Into<PeerRef>) -> Result<ChannelStatistics> {
        let channel = self.resolve_channel(&chat_id.into()).await?;
        let stats = self.invoke(&functions::stats::GetBroadcastStats { dark: false, channel }).await?;
        Ok(ChannelStatistics::parse(stats))
    }
}
