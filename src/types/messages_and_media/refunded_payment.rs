use crate::raw::{base, types};

/// A payment that was refunded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefundedPayment {
    /// Currency code of the payment.
    pub currency: String,
    /// Amount in the smallest units of `currency`.
    pub total_amount: i64,
    /// The payload the bot set on the invoice.
    pub invoice_payload: String,
    /// Telegram's id of the charge.
    pub telegram_payment_charge_id: String,
    /// The payment provider's id of the charge.
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
