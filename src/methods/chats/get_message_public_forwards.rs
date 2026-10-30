use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::{Message, PeerRef};

/// Public reposts of a message. Reposts to stories are left out: they are not messages.
pub struct GetMessagePublicForwards<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    pages: OffsetPages<Message>,
}

impl Client {
    pub fn get_message_public_forwards(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
    ) -> GetMessagePublicForwards<'_> {
        GetMessagePublicForwards { client: self, chat_id: chat_id.into(), message_id, pages: OffsetPages::new(100) }
    }
}

impl GetMessagePublicForwards<'_> {
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    pub async fn next(&mut self) -> Result<Option<Message>> {
        loop {
            if let Some(message) = self.pages.pop() {
                return Ok(Some(message));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            self.chat_id = PeerRef::Id(self.client.resolve(&self.chat_id).await?.0);
            let request = functions::stats::GetMessagePublicForwards {
                channel: self.client.resolve_channel(&self.chat_id).await?,
                msg_id: self.message_id,
                offset: self.pages.next_offset().unwrap_or_default().into(),
                limit,
            };
            let base::stats::PublicForwards::PublicForwards(page) = self.client.invoke(&request).await?;
            let page = *page;
            let entities = Entities::new(self.client, page.users, page.chats);
            let client = self.client;
            self.pages.refill(page.forwards, page.next_offset, |forwards| {
                forwards
                    .into_iter()
                    .filter_map(|f| match f {
                        base::PublicForward::PublicForwardMessage(f) => Message::from_raw(client, f.message, &entities),
                        base::PublicForward::PublicForwardStory(_) => None,
                    })
                    .collect()
            });
        }
    }
}
