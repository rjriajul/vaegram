use super::edit_forum_topic::GENERAL_TOPIC_ID;

use crate::Client;
use crate::errors::Result;
use crate::types::PeerRef;

impl Client {
    pub async fn edit_general_forum_topic(&self, chat_id: impl Into<PeerRef>, title: impl Into<String>) -> Result<()> {
        self.edit_forum_topic(chat_id, GENERAL_TOPIC_ID).title(title).await
    }
}
