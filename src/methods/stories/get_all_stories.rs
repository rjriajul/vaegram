use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Story;

impl Client {
    pub async fn get_all_stories(&self, hidden: bool) -> Result<Vec<Story>> {
        let mut stories = Vec::new();
        let mut state = None;
        loop {
            let request = functions::stories::GetAllStories { next: state.is_some(), hidden, state: state.take() };
            let base::stories::AllStories::AllStories(page) = self.invoke(&request).await? else { break };
            let entities = Entities::new(self, page.users, page.chats);
            for base::PeerStories::PeerStories(peer) in page.peer_stories {
                stories.extend(peer.stories.into_iter().filter_map(|item| Story::parse(&peer.peer, item, &entities)));
            }
            if !page.has_more {
                break;
            }
            state = Some(page.state);
        }
        Ok(stories)
    }
}
