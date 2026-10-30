use super::create_business_chat_link::check_slug;
use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn delete_business_chat_link(&self, slug: impl Into<String>) -> Result<bool> {
        let slug = slug.into();
        check_slug(&slug)?;
        self.invoke(&functions::account::DeleteBusinessChatLink { slug }).await
    }
}
