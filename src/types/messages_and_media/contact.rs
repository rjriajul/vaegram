use crate::raw::types;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contact {
    pub phone_number: String,
    pub first_name: String,
    pub last_name: Option<String>,
    pub user_id: Option<i64>,
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
