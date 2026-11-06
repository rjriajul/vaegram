/// A contact to import by phone number.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct InputPhoneContact {
    /// The phone number, in international format.
    pub phone_number: String,
    /// The contact's first name.
    pub first_name: String,
    /// The contact's last name. Empty by default.
    pub last_name: String,
}

impl InputPhoneContact {
    /// Creates a contact with an empty last name.
    pub fn new(phone_number: impl Into<String>, first_name: impl Into<String>) -> InputPhoneContact {
        InputPhoneContact { phone_number: phone_number.into(), first_name: first_name.into(), last_name: String::new() }
    }

    /// Sets the contact's last name, empty by default.
    #[must_use]
    pub fn last_name(mut self, last_name: impl Into<String>) -> Self {
        self.last_name = last_name.into();
        self
    }
}
