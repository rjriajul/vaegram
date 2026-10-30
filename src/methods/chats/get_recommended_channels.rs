use super::get_similar_channels::parse_channels;
use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::Chat;

impl Client {
    pub async fn get_recommended_channels(&self) -> Result<Vec<Chat>> {
        let found = self.invoke(&functions::channels::GetChannelRecommendations { channel: None }).await?;
        Ok(parse_channels(self, found))
    }
}
