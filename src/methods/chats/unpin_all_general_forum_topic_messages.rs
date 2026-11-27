use super::edit_forum_topic::GENERAL_TOPIC_ID;

use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    /// Unpins every pinned message in the General topic of a forum.
    pub async fn unpin_all_general_forum_topic_messages(&self, chat_id: impl Into<PeerRef>) -> Result<()> {
        // The Bot API unpins the General topic with a method of its own.
        #[cfg(feature = "bot-api")]
        if let Some(bot) = self.0.bot_api.as_deref() {
            return bot.unpin_all_general_forum_topic_messages(self.peer_id(chat_id.into()).await?).await;
        }
        self.unpin_all_forum_topic_messages(chat_id, GENERAL_TOPIC_ID).await
    }
}
