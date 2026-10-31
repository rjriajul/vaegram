use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{DirectMessagesChatTopic, PeerRef};

impl Client {
    /// `chat_id` is the direct messages chat, not its channel; `topic_ids` are the users writing to it.
    pub async fn get_direct_messages_topics_by_id(
        &self,
        chat_id: impl Into<PeerRef>,
        topic_ids: &[i64],
    ) -> Result<Vec<DirectMessagesChatTopic>> {
        if topic_ids.is_empty() {
            return Err(Error::InvalidArgument("get_direct_messages_topics_by_id needs at least one topic id"));
        }
        let request = functions::messages::GetSavedDialogsByID {
            parent_peer: Some(self.resolve_peer(chat_id).await?),
            ids: self.resolve_peers(topic_ids.iter().copied()).await?,
        };
        let (dialogs, users, chats) = match self.invoke(&request).await? {
            base::messages::SavedDialogs::SavedDialogs(d) => (d.dialogs, d.users, d.chats),
            base::messages::SavedDialogs::SavedDialogsSlice(d) => (d.dialogs, d.users, d.chats),
            base::messages::SavedDialogs::SavedDialogsNotModified(_) => return Ok(Vec::new()),
        };
        let entities = Entities::new(self, users, chats);
        Ok(dialogs
            .iter()
            .filter_map(|d| match d {
                base::SavedDialog::MonoForumDialog(d) => Some(DirectMessagesChatTopic::parse(d, &entities)),
                base::SavedDialog::SavedDialog(_) => None,
            })
            .collect())
    }
}
