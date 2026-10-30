#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputPhoneContact {
    pub phone_number: String,
    pub first_name: String,
    pub last_name: String,
}

impl InputPhoneContact {
    pub fn new(phone_number: impl Into<String>, first_name: impl Into<String>) -> InputPhoneContact {
        InputPhoneContact { phone_number: phone_number.into(), first_name: first_name.into(), last_name: String::new() }
    }
}
