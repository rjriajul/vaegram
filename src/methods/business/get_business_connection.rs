use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::BusinessConnection;

impl Client {
    pub async fn get_business_connection(&self, business_connection_id: &str) -> Result<BusinessConnection> {
        let request = functions::account::GetBotBusinessConnection { connection_id: business_connection_id.into() };
        let (updates, users, chats) = match self.invoke(&request).await? {
            base::Updates::Updates(u) => (u.updates, u.users, u.chats),
            base::Updates::UpdatesCombined(u) => (u.updates, u.users, u.chats),
            base::Updates::UpdateShort(u) => (vec![u.update], Vec::new(), Vec::new()),
            _ => return Err(Error::Unexpected("Telegram sent no business connection")),
        };
        let entities = Entities::new(self, users, chats);
        updates
            .into_iter()
            .find_map(|u| match u {
                base::Update::UpdateBotBusinessConnect(u) => {
                    Some(BusinessConnection::parse(self, u.connection, &entities))
                }
                _ => None,
            })
            .ok_or(Error::Unexpected("Telegram sent no business connection"))
    }
}
