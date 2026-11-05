/// Why the Stars or TON paid for a suggested post were refunded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuggestedPostRefundReason {
    /// The post was deleted, and Telegram refunded the payer.
    PostDeleted,
    /// The payer asked for the refund.
    PaymentRefunded,
}
