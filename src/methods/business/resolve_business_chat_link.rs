use super::create_business_chat_link::check_slug;
use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::BusinessChatLinkInfo;

impl Client {
    pub async fn resolve_business_chat_link(&self, slug: impl Into<String>) -> Result<BusinessChatLinkInfo> {
        let slug = slug.into();
        check_slug(&slug)?;
        let request = functions::account::ResolveBusinessChatLink { slug };
        let base::account::ResolvedBusinessChatLinks::ResolvedBusinessChatLinks(mut r) = self.invoke(&request).await?;
        let entities = Entities::new(self, std::mem::take(&mut r.users), std::mem::take(&mut r.chats));
        Ok(BusinessChatLinkInfo::parse(*r, &entities))
    }
}
