use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::Message;

/// Request built by [`Client::get_quick_reply_messages`]. Awaiting it returns the shortcut's messages.
#[must_use = "a request does nothing until it is awaited"]
pub struct GetQuickReplyMessages<'a> {
    client: &'a Client,
    shortcut_id: i32,
    message_ids: Option<Vec<i32>>,
}

impl Client {
    /// Returns the messages of the quick reply shortcut `shortcut_id`, or only some of them with
    /// [`message_ids`](GetQuickReplyMessages::message_ids).
    ///
    /// The messages are in the account's own chat, with ids from a sequence of their own that only quick reply
    /// methods such as [`send_quick_reply_messages`](Client::send_quick_reply_messages) and
    /// [`edit_quick_reply_message`](Client::edit_quick_reply_message) understand.
    pub fn get_quick_reply_messages(&self, shortcut_id: i32) -> GetQuickReplyMessages<'_> {
        GetQuickReplyMessages { client: self, shortcut_id, message_ids: None }
    }
}

impl GetQuickReplyMessages<'_> {
    /// Returns only these messages of the shortcut. Awaiting fails with `Error::InvalidArgument` if `ids` is empty.
    pub fn message_ids(mut self, ids: impl IntoIterator<Item = i32>) -> Self {
        self.message_ids = Some(ids.into_iter().collect());
        self
    }

    async fn send(self) -> Result<Vec<Message>> {
        if self.message_ids.as_ref().is_some_and(Vec::is_empty) {
            return Err(Error::InvalidArgument("get_quick_reply_messages needs at least one message id"));
        }
        let request =
            functions::messages::GetQuickReplyMessages { shortcut_id: self.shortcut_id, id: self.message_ids, hash: 0 };
        let found = self.client.invoke(&request).await?;
        let (messages, entities) = Message::unpack(self.client, found);
        Ok(messages.into_iter().filter_map(|m| Message::from_raw(self.client, m, &entities)).collect())
    }
}

into_future!(GetQuickReplyMessages, Vec<Message>);
