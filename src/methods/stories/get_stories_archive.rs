use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{PeerRef, Story};
use crate::utils::get_peer;

const PAGE_SIZE: usize = 100;

#[must_use = "call .next() to fetch stories"]
pub struct StoryPages<'a> {
    client: &'a Client,
    chat: PeerRef,
    archive: bool,
    offset_id: i32,
    remaining: usize,
    buffer: std::vec::IntoIter<Story>,
    done: bool,
}

impl Client {
    /// Only the chat itself, or a channel's admins, can read its archive.
    pub fn get_stories_archive(&self, chat_id: impl Into<PeerRef>) -> StoryPages<'_> {
        StoryPages::new(self, chat_id.into(), true)
    }
}

impl<'a> StoryPages<'a> {
    pub(crate) fn new(client: &'a Client, chat: PeerRef, archive: bool) -> Self {
        let buffer = Vec::new().into_iter();
        StoryPages { client, chat, archive, offset_id: 0, remaining: usize::MAX, buffer, done: false }
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.remaining = limit;
        self
    }

    pub fn offset_id(mut self, story_id: i32) -> Self {
        self.offset_id = story_id;
        self
    }

    pub async fn next(&mut self) -> Result<Option<Story>> {
        loop {
            if self.remaining == 0 {
                return Ok(None);
            }
            if let Some(story) = self.buffer.next() {
                self.remaining -= 1;
                return Ok(Some(story));
            }
            if self.done {
                return Ok(None);
            }
            let (chat_id, peer) = self.client.resolve(&self.chat).await?;
            let (offset_id, limit) = (self.offset_id, self.remaining.min(PAGE_SIZE) as i32);
            let base::stories::Stories::Stories(page) = if self.archive {
                self.client.invoke(&functions::stories::GetStoriesArchive { peer, offset_id, limit }).await?
            } else {
                self.client.invoke(&functions::stories::GetPinnedStories { peer, offset_id, limit }).await?
            };
            self.fill(chat_id, page.stories, page.users, page.chats);
        }
    }

    fn fill(&mut self, chat_id: i64, items: Vec<base::StoryItem>, users: Vec<base::User>, chats: Vec<base::Chat>) {
        let last = items.last().map(|item| match item {
            base::StoryItem::StoryItemDeleted(s) => s.id,
            base::StoryItem::StoryItemSkipped(s) => s.id,
            base::StoryItem::StoryItem(s) => s.id,
        });
        let previous = self.offset_id;
        match last {
            // An offset that doesn't move down means Telegram has nothing older to give.
            Some(id) if previous == 0 || id < previous => self.offset_id = id,
            _ => {
                self.done = true;
                return;
            }
        }
        let mut stories = Story::parse_list(self.client, &get_peer(chat_id), items, users, chats);
        stories.retain(|s| previous == 0 || s.id < previous);
        self.buffer = stories.into_iter();
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{base, types};

    fn deleted(id: i32) -> base::StoryItem {
        base::StoryItem::StoryItemDeleted(types::StoryItemDeleted { id })
    }

    fn skipped(id: i32) -> base::StoryItem {
        let story = types::StoryItemSkipped { close_friends: false, live: false, id, date: id, expire_date: id };
        base::StoryItem::StoryItemSkipped(Box::new(story))
    }

    #[test]
    fn pages_walk_down_and_stop() {
        let client = Client::offline();
        let mut pages = client.get_stories_archive(42).limit(10);
        pages.fill(42, vec![skipped(9), deleted(8), skipped(7)], Vec::new(), Vec::new());
        assert_eq!(pages.offset_id, 7);
        assert_eq!(pages.buffer.as_slice().iter().map(|s| s.id).collect::<Vec<_>>(), [9, 7]);
        assert_eq!(pages.buffer.as_slice()[0].chat.id, 42);

        pages.fill(42, vec![skipped(7), skipped(5)], Vec::new(), Vec::new());
        assert_eq!(pages.buffer.as_slice().iter().map(|s| s.id).collect::<Vec<_>>(), [5]);
        assert!(!pages.done);

        pages.fill(42, Vec::new(), Vec::new(), Vec::new());
        assert!(pages.done);
    }
}
