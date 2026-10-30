use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::BusinessChatLink;

impl Client {
    pub async fn get_business_chat_links(&self) -> Result<Vec<BusinessChatLink>> {
        let base::account::BusinessChatLinks::BusinessChatLinks(l) =
            self.invoke(&functions::account::GetBusinessChatLinks {}).await?;
        let l = *l;
        let entities = Entities::new(self, l.users, l.chats);
        Ok(l.links.into_iter().map(|link| BusinessChatLink::parse(link, &entities)).collect())
    }
}
