use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn delete_contacts(&self, user_ids: impl IntoIterator<Item = impl Into<PeerRef>>) -> Result<()> {
        let user_ids: Vec<PeerRef> = user_ids.into_iter().map(Into::into).collect();
        if user_ids.is_empty() {
            return Err(Error::InvalidArgument("delete_contacts needs at least one user"));
        }
        let mut id = Vec::with_capacity(user_ids.len());
        for user_id in user_ids {
            id.push(self.resolve_user(&user_id).await?);
        }
        self.invoke(&functions::contacts::DeleteContacts { id }).await?;
        Ok(())
    }
}
