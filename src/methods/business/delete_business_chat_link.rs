use super::create_business_chat_link::check_slug;
use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

impl Client {
    /// Deletes a business chat link by its slug. Fails with `Error::InvalidArgument` if `slug` is empty.
    pub async fn delete_business_chat_link(&self, slug: impl Into<String>) -> Result<()> {
        let slug = slug.into();
        check_slug(&slug)?;
        accepted(self.invoke(&functions::account::DeleteBusinessChatLink { slug }).await?)
    }
}
