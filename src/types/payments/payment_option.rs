/// Another way to pay a card form, such as a wallet app's checkout page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentOption {
    pub title: String,
    pub url: String,
}
