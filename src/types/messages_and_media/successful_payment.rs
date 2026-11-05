use crate::raw::{base, types};
use crate::types::OrderInfo;

/// A payment that went through, received by the bot or account that issued the invoice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuccessfulPayment {
    /// Three-letter currency code of the invoice, or `XTR` for Stars.
    pub currency: String,
    /// Total price in the smallest units of `currency`.
    pub total_amount: i64,
    /// Payload set when the invoice was created, decoded as UTF-8 (invalid bytes are replaced).
    pub invoice_payload: String,
    /// Unix time when the subscription ends. `None` for payments that aren't subscriptions.
    pub subscription_expiration_date: Option<i32>,
    /// `true` for a recurring payment, first or repeated.
    pub is_recurring: bool,
    /// `true` for the first payment of a recurring series.
    pub is_first_recurring: bool,
    /// Id of the shipping option the user chose. `None` if the invoice had none.
    pub shipping_option_id: Option<String>,
    /// Order info the user gave. `None` if none was requested.
    pub order_info: Option<OrderInfo>,
    /// Telegram's id of the payment.
    pub telegram_payment_charge_id: String,
    /// The payment provider's id of the payment.
    pub provider_payment_charge_id: String,
}

impl SuccessfulPayment {
    pub(crate) fn parse(raw: &types::MessageActionPaymentSentMe) -> SuccessfulPayment {
        let base::PaymentCharge::PaymentCharge(charge) = &raw.charge;
        SuccessfulPayment {
            currency: raw.currency.clone(),
            total_amount: raw.total_amount,
            invoice_payload: String::from_utf8_lossy(&raw.payload).into_owned(),
            subscription_expiration_date: raw.subscription_until_date,
            is_recurring: raw.recurring_init || raw.recurring_used,
            is_first_recurring: raw.recurring_init,
            shipping_option_id: raw.shipping_option_id.clone(),
            order_info: raw.info.as_ref().map(OrderInfo::parse),
            telegram_payment_charge_id: charge.id.clone(),
            provider_payment_charge_id: charge.provider_charge_id.clone(),
        }
    }
}
