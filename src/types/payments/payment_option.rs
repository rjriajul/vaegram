/// Another way to pay a card form, such as a wallet app's checkout page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaymentOption {
    /// Name of the option.
    pub title: String,
    /// Page where the payment is made.
    pub url: String,
}
