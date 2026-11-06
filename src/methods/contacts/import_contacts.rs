use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{InputPhoneContact, User};

impl Client {
    /// Imports phone contacts and returns the users Telegram matched to them.
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

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::InputPhoneContact;

    #[tokio::test]
    async fn every_name_reaches_the_request() {
        let (client, mut rx) = Client::recording();
        let contacts = [InputPhoneContact::new("+100", "Ann"), InputPhoneContact::new("+200", "Bo").last_name("Lee")];
        let _ = client.import_contacts(&contacts).await;
        let contact = |client_id, phone: &str, first_name: &str, last_name: &str| {
            base::InputContact::InputPhoneContact(Box::new(types::InputPhoneContact {
                client_id,
                phone: phone.into(),
                first_name: first_name.into(),
                last_name: last_name.into(),
                note: None,
            }))
        };
        let contacts = vec![contact(0, "+100", "Ann", ""), contact(1, "+200", "Bo", "Lee")];
        assert_eq!(rx.try_recv().unwrap(), functions::contacts::ImportContacts { contacts }.to_bytes());
    }
}
