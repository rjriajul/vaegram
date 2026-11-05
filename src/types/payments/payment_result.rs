/// What paying a form came to.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PaymentResult {
    /// The payment went through.
    Paid,
    /// The provider wants the customer to finish the payment (for example a 3-D Secure check) at this page.
    VerificationNeeded {
        /// The page to finish the payment at.
        url: String,
    },
}

impl PaymentResult {
    /// Returns `true` if the payment went through.
    pub fn success(&self) -> bool {
        matches!(self, PaymentResult::Paid)
    }

    /// Returns the page to finish the payment at, or `None` unless this is
    /// [`PaymentResult::VerificationNeeded`].
    pub fn verification_url(&self) -> Option<&str> {
        match self {
            PaymentResult::VerificationNeeded { url } => Some(url),
            PaymentResult::Paid => None,
        }
    }
}
