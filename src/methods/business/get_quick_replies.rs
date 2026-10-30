use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::QuickReply;

impl Client {
    pub async fn get_quick_replies(&self) -> Result<Vec<QuickReply>> {
        match self.invoke(&functions::messages::GetQuickReplies { hash: 0 }).await? {
            base::messages::QuickReplies::QuickReplies(q) => {
                Ok(q.quick_replies.into_iter().map(QuickReply::parse).collect())
            }
            base::messages::QuickReplies::QuickRepliesNotModified => Ok(Vec::new()),
        }
    }
}
