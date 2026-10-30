use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{MessageStatistics, PeerRef};

impl Client {
    pub async fn get_message_statistics(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
    ) -> Result<MessageStatistics> {
        let channel = self.resolve_channel(&chat_id.into()).await?;
        let request = functions::stats::GetMessageStats { dark: false, channel, msg_id: message_id };
        let base::stats::MessageStats::MessageStats(s) = self.invoke(&request).await?;
        Ok(MessageStatistics::parse(s.views_graph, s.reactions_by_emotion_graph))
    }
}
