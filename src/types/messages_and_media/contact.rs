use crate::raw::types;

/// A shared phone contact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contact {
    /// Phone number as the sender wrote it.
    pub phone_number: String,
    /// Contact's first name.
    pub first_name: String,
    /// Contact's last name. `None` if empty.
    pub last_name: Option<String>,
    /// Telegram id of the contact. `None` if the number isn't linked to an account.
    pub user_id: Option<i64>,
    /// The contact as a vCard. `None` if empty.
    pub vcard: Option<String>,
}

impl Contact {
    pub(crate) fn parse(contact: &types::MessageMediaContact) -> Contact {
        let some = |s: &String| (!s.is_empty()).then(|| s.clone());
        Contact {
            phone_number: contact.phone_number.clone(),
            first_name: contact.first_name.clone(),
            last_name: some(&contact.last_name),
            user_id: (contact.user_id != 0).then_some(contact.user_id),
            vcard: some(&contact.vcard),
        }
    }
}
