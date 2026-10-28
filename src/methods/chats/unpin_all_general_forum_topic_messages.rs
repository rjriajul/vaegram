use super::edit_forum_topic::GENERAL_TOPIC_ID;

use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    pub async fn unpin_all_general_forum_topic_messages(&self, chat_id: impl Into<PeerRef>) -> Result<()> {
        self.unpin_all_forum_topic_messages(chat_id, GENERAL_TOPIC_ID).await
    }
}
