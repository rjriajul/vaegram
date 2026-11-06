use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::{Message, PeerRef};

/// Public reposts of a channel message. Awaiting it collects the reposts into a `Vec`, up to
/// [`limit`](GetMessagePublicForwards::limit); [`next`](GetMessagePublicForwards::next) reads them one at a time.
///
/// Reposts to stories are left out because they are not messages.
///
/// Set `limit` before awaiting; without it this reads every repost (one request per 100).
#[must_use = "a pager does nothing until it is awaited or read with next()"]
pub struct GetMessagePublicForwards<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    pages: OffsetPages<Message>,
}

impl Client {
    /// Starts reading the public reposts of a channel message, 100 per request.
    pub fn get_message_public_forwards(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
    ) -> GetMessagePublicForwards<'_> {
        GetMessagePublicForwards { client: self, chat_id: chat_id.into(), message_id, pages: OffsetPages::new(100) }
    }
}

impl GetMessagePublicForwards<'_> {
    /// Stops after this many reposts in total. The default reads them all.
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    /// Returns the next repost, or `None` once the list or the limit is exhausted.
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
            // Stories stay in as `None`, so a page of only stories doesn't end the list.
            let mut forwards: Vec<Option<Message>> = page
                .forwards
                .into_iter()
                .map(|f| match f {
                    base::PublicForward::PublicForwardMessage(f) => Message::from_raw(client, f.message, &entities),
                    base::PublicForward::PublicForwardStory(_) => None,
                })
                .collect();
            client.prefetch_found(&mut forwards).await;
            self.pages.refill(forwards, page.next_offset, |forwards| forwards.into_iter().flatten().collect());
        }
    }
}

pager!(GetMessagePublicForwards, Message);
