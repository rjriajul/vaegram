use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::{PeerRef, StoryViewer};

pub struct GetStoryViewers<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    story_id: i32,
    pages: OffsetPages<StoryViewer>,
}

impl Client {
    pub fn get_story_viewers(&self, chat_id: impl Into<PeerRef>, story_id: i32) -> GetStoryViewers<'_> {
        GetStoryViewers { client: self, chat_id: chat_id.into(), story_id, pages: OffsetPages::new(100) }
    }
}

impl GetStoryViewers<'_> {
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    pub async fn next(&mut self) -> Result<Option<StoryViewer>> {
        loop {
            if let Some(viewer) = self.pages.pop() {
                return Ok(Some(viewer));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let (chat_id, peer) = self.client.resolve(&self.chat_id).await?;
            self.chat_id = PeerRef::Id(chat_id);
            let request = functions::stories::GetStoryViewsList {
                just_contacts: false,
                reactions_first: false,
                forwards_first: false,
                peer,
                q: None,
                id: self.story_id,
                offset: self.pages.next_offset().unwrap_or_default().into(),
                limit,
            };
            let base::stories::StoryViewsList::StoryViewsList(page) = self.client.invoke(&request).await?;
            let entities = Entities::new(self.client, page.users, page.chats);
            // Public forwards and reposts share the list but aren't viewers.
            let parse = |views: Vec<_>| views.into_iter().filter_map(|v| StoryViewer::parse(v, &entities)).collect();
            self.pages.refill(page.views, page.next_offset, parse);
        }
    }
}
