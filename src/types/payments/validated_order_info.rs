use crate::types::ShippingOption;

/// The provider's answer to an order's contact and shipping details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedOrderInfo {
    /// Pass it to [`SendPaymentForm::order_info_id`](crate::methods::SendPaymentForm::order_info_id).
    pub id: Option<String>,
    /// The ways to ship the order; empty when the form isn't flexible.
    pub shipping_options: Vec<ShippingOption>,
}
