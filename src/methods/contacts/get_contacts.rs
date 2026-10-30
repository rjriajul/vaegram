use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::User;

impl Client {
    pub async fn get_contacts(&self) -> Result<Vec<User>> {
        let found = match self.invoke(&functions::contacts::GetContacts { hash: 0 }).await? {
            base::contacts::Contacts::Contacts(found) => found,
            // Only sent back for a non-zero hash.
            base::contacts::Contacts::ContactsNotModified => return Ok(Vec::new()),
        };
        let entities = Entities::new(self, found.users, Vec::new());
        Ok(found.contacts.iter().map(|base::Contact::Contact(c)| User::parse(c.user_id, &entities)).collect())
    }
}
