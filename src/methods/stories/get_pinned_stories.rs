use crate::Client;
use crate::types::PeerRef;

use super::get_stories_archive::StoryPages;

impl Client {
    pub fn get_pinned_stories(&self, chat_id: impl Into<PeerRef>) -> StoryPages<'_> {
        StoryPages::new(self, chat_id.into(), false)
    }
}
