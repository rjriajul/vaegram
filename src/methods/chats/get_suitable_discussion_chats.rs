use super::get_similar_channels::parse_channels;
use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::Chat;

impl Client {
    pub async fn get_suitable_discussion_chats(&self) -> Result<Vec<Chat>> {
        let found = self.invoke(&functions::channels::GetGroupsForDiscussion {}).await?;
        Ok(parse_channels(self, found))
    }
}
