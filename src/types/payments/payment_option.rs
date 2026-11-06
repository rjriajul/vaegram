/// Another way to pay a card form, such as a wallet app's checkout page.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PaymentOption {
    /// Label the option is shown under.
    pub title: String,
    /// Page where the payment is made with this option.
    pub url: String,
}
