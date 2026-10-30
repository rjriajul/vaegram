use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::{PeerReaction, PeerRef, ReactionType};

pub struct GetMessageReactionsList<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    reaction: Option<ReactionType>,
    pages: OffsetPages<PeerReaction>,
}

impl Client {
    pub fn get_message_reactions_list(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
    ) -> GetMessageReactionsList<'_> {
        let pages = OffsetPages::new(100);
        GetMessageReactionsList { client: self, chat_id: chat_id.into(), message_id, reaction: None, pages }
    }
}

impl GetMessageReactionsList<'_> {
    pub fn reaction(mut self, reaction: impl Into<ReactionType>) -> Self {
        self.reaction = Some(reaction.into());
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    pub async fn next(&mut self) -> Result<Option<PeerReaction>> {
        loop {
            if let Some(reaction) = self.pages.pop() {
                return Ok(Some(reaction));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let (chat_id, peer) = self.client.resolve(&self.chat_id).await?;
            self.chat_id = PeerRef::Id(chat_id);
            let request = functions::messages::GetMessageReactionsList {
                peer,
                id: self.message_id,
                reaction: self.reaction.as_ref().map(ReactionType::write),
                offset: self.pages.next_offset().map(Into::into),
                limit,
            };
            let base::messages::MessageReactionsList::MessageReactionsList(page) = self.client.invoke(&request).await?;
            Entities::new(self.client, page.users, page.chats);
            self.pages.refill(page.reactions, page.next_offset, |r| r.iter().filter_map(PeerReaction::parse).collect());
        }
    }
}
