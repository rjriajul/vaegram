use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::{PeerRef, PollVote};

pub struct GetPollVotes<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    option: Option<Vec<u8>>,
    pages: OffsetPages<PollVote>,
}

impl Client {
    pub fn get_poll_votes(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> GetPollVotes<'_> {
        GetPollVotes { client: self, chat_id: chat_id.into(), message_id, option: None, pages: OffsetPages::new(50) }
    }
}

impl GetPollVotes<'_> {
    pub fn option(mut self, option: impl Into<Vec<u8>>) -> Self {
        self.option = Some(option.into());
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    pub async fn next(&mut self) -> Result<Option<PollVote>> {
        loop {
            if let Some(vote) = self.pages.pop() {
                return Ok(Some(vote));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let (chat_id, peer) = self.client.resolve(&self.chat_id).await?;
            self.chat_id = PeerRef::Id(chat_id);
            let request = functions::messages::GetPollVotes {
                peer,
                id: self.message_id,
                option: self.option.clone(),
                offset: self.pages.next_offset().map(Into::into),
                limit,
            };
            let base::messages::VotesList::VotesList(page) = self.client.invoke(&request).await?;
            Entities::new(self.client, page.users, page.chats);
            let queried = request.option.as_deref();
            self.pages
                .refill(page.votes, page.next_offset, |v| v.into_iter().map(|v| PollVote::parse(v, queried)).collect());
        }
    }
}
