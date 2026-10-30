use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{InputPhoneContact, User};

impl Client {
    pub async fn import_contacts(&self, contacts: &[InputPhoneContact]) -> Result<Vec<User>> {
        let contacts = contacts
            .iter()
            .zip(0..)
            .map(|(c, client_id)| {
                base::InputContact::InputPhoneContact(Box::new(types::InputPhoneContact {
                    client_id,
                    phone: c.phone_number.clone(),
                    first_name: c.first_name.clone(),
                    last_name: c.last_name.clone(),
                    note: None,
                }))
            })
            .collect();
        let base::contacts::ImportedContacts::ImportedContacts(imported) =
            self.invoke(&functions::contacts::ImportContacts { contacts }).await?;
        let entities = Entities::new(self, imported.users, Vec::new());
        Ok(imported
            .imported
            .iter()
            .map(|base::ImportedContact::ImportedContact(c)| User::parse(c.user_id, &entities))
            .collect())
    }
}
