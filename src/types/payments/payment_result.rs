/// What paying a form came to.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PaymentResult {
    Paid,
    /// The provider wants the customer to finish the payment (for example a 3-D Secure check) at this page.
    VerificationNeeded {
        url: String,
    },
}

impl PaymentResult {
    pub fn success(&self) -> bool {
        matches!(self, PaymentResult::Paid)
    }

    pub fn verification_url(&self) -> Option<&str> {
        match self {
            PaymentResult::VerificationNeeded { url } => Some(url),
            PaymentResult::Paid => None,
        }
    }
}
