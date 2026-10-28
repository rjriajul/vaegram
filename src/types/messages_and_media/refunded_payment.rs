use crate::raw::{base, types};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefundedPayment {
    pub currency: String,
    pub total_amount: i64,
    pub invoice_payload: String,
    pub telegram_payment_charge_id: String,
    pub provider_payment_charge_id: String,
}

impl RefundedPayment {
    pub(crate) fn parse(raw: &types::MessageActionPaymentRefunded) -> RefundedPayment {
        let base::PaymentCharge::PaymentCharge(charge) = &raw.charge;
        RefundedPayment {
            currency: raw.currency.clone(),
            total_amount: raw.total_amount,
            invoice_payload: raw.payload.as_deref().map(String::from_utf8_lossy).unwrap_or_default().into_owned(),
            telegram_payment_charge_id: charge.id.clone(),
            provider_payment_charge_id: charge.provider_charge_id.clone(),
        }
    }
}
