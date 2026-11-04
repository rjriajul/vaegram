/// What a [`PaymentForm`](crate::types::PaymentForm) pays for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PaymentFormType {
    /// A bot's invoice paid by card.
    Regular,
    /// An invoice paid in Telegram Stars.
    Stars,
    /// A Stars invoice that charges again every period.
    StarSubscription,
    /// A star gift.
    StarGift,
}
