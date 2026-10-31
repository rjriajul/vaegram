use super::get_similar_channels::parse_channels;
use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::Chat;

impl Client {
    pub async fn get_personal_channels(&self) -> Result<Vec<Chat>> {
        let request = functions::channels::GetAdminedPublicChannels {
            by_location: false,
            check_limit: false,
            for_personal: true,
            for_community_peer: false,
        };
        let found = self.invoke(&request).await?;
        Ok(parse_channels(self, found))
    }
}
