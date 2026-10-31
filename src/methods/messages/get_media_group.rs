use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::types;
use crate::types::{Message, PeerRef};

impl Client {
    /// The album the message belongs to, in order.
    pub async fn get_media_group(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> Result<Vec<Message>> {
        let (group, entities) = self.media_group(chat_id, message_id).await?;
        Ok(group.into_iter().map(|m| Message::parse(self, m, &entities)).collect())
    }

    pub(super) async fn media_group(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
    ) -> Result<(Vec<Box<types::Message>>, Entities)> {
        if message_id <= 0 {
            return Err(Error::InvalidArgument("message ids start at 1"));
        }
        // An album holds at most 10 messages, so 9 ids either side of one of them cover the rest.
        let ids: Vec<i32> = ((message_id - 9).max(1)..=message_id.saturating_add(9)).collect();
        let (found, entities) = self.get_raw_messages(chat_id, &ids).await?;
        let mut found: Vec<_> = found.into_iter().flatten().collect();
        let anchor =
            found.iter().find(|m| m.id == message_id).ok_or(Error::InvalidArgument("no message with this id"))?;
        let group = anchor.grouped_id.ok_or(Error::InvalidArgument("this message isn't part of a media group"))?;
        found.retain(|m| m.grouped_id == Some(group));
        Ok((found, entities))
    }
}
