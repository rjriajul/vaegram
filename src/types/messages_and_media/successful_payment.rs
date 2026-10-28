use crate::raw::{base, types};
use crate::types::OrderInfo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuccessfulPayment {
    pub currency: String,
    pub total_amount: i64,
    pub invoice_payload: String,
    pub subscription_expiration_date: Option<i32>,
    pub is_recurring: bool,
    pub is_first_recurring: bool,
    pub shipping_option_id: Option<String>,
    pub order_info: Option<OrderInfo>,
    pub telegram_payment_charge_id: String,
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
