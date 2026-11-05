/// Why the Stars or TON paid for a suggested post were refunded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuggestedPostRefundReason {
    /// The post was deleted soon after going out, or removed from the schedule before it did.
    PostDeleted,
    /// The payer refunded the payment.
    PaymentRefunded,
}
