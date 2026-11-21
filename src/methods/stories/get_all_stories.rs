use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Story;

impl Client {
    /// Returns the active stories of every chat you follow, fetching all pages.
    ///
    /// With `hidden` set, returns the stories of chats you moved to the hidden list instead of the main one.
    pub async fn get_all_stories(&self, hidden: bool) -> Result<Vec<Story>> {
        let mut stories = Vec::new();
        let mut state: Option<String> = None;
        loop {
            let request = functions::stories::GetAllStories { next: state.is_some(), hidden, state: state.clone() };
            let base::stories::AllStories::AllStories(page) = self.invoke(&request).await? else { break };
            let entities = Entities::new(self, page.users, page.chats);
            for base::PeerStories::PeerStories(peer) in page.peer_stories {
                stories.extend(
                    peer.stories.into_iter().filter_map(|item| Story::parse(self, &peer.peer, item, &entities)),
                );
            }
            // An empty or repeated state would fetch the same page forever.
            if !page.has_more || page.state.is_empty() || state.as_deref() == Some(page.state.as_str()) {
                break;
            }
            state = Some(page.state);
        }
        Ok(stories)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, types};

    fn page(state: &str) -> Vec<u8> {
        let stealth = types::StoriesStealthMode { active_until_date: None, cooldown_until_date: None };
        let page = types::stories::AllStories {
            has_more: true,
            count: 1,
            state: state.into(),
            peer_stories: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
            stealth_mode: base::StoriesStealthMode::StoriesStealthMode(Box::new(stealth)),
        };
        base::stories::AllStories::AllStories(Box::new(page)).to_bytes()
    }

    #[tokio::test]
    async fn a_repeated_or_empty_state_ends_the_list() {
        let (client, mut rx) = Client::scripted(vec![Ok(page("a")), Ok(page("a")), Ok(page("a"))]);
        assert!(client.get_all_stories(false).await.unwrap().is_empty());
        assert_eq!(std::iter::from_fn(|| rx.try_recv().ok()).count(), 2);

        let (client, mut rx) = Client::scripted(vec![Ok(page("")), Ok(page(""))]);
        assert!(client.get_all_stories(false).await.unwrap().is_empty());
        assert_eq!(std::iter::from_fn(|| rx.try_recv().ok()).count(), 1);
    }
}
