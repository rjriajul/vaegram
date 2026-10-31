use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::{LabeledPrice, User};

/// A price Telegram quoted for an invoice. Only [`Client::get_payment_form`] makes one, and paying it takes it by
/// value, so each form is paid at most once on purpose:
///
/// ```compile_fail
/// let form = vaegram::types::PaymentForm {
///     id: 1,
///     bot: None,
///     title: None,
///     description: None,
///     currency: "XTR".into(),
///     prices: Vec::new(),
///     total_amount: 1,
///     subscription_period: None,
///     is_test: false,
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PaymentForm {
    pub id: i64,
    pub bot: Option<User>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub currency: String,
    pub prices: Vec<LabeledPrice>,
    pub total_amount: i64,
    /// Seconds between charges: paying this form starts a subscription.
    pub subscription_period: Option<i32>,
    pub is_test: bool,
    pub(crate) invoice: base::InputInvoice,
}

impl PaymentForm {
    pub(crate) fn parse(client: &Client, raw: base::payments::PaymentForm, invoice: base::InputInvoice) -> PaymentForm {
        let (id, bot, title, description, terms) = match raw {
            base::payments::PaymentForm::PaymentForm(f) => {
                let f = *f;
                let entities = Entities::new(client, f.users, Vec::new());
                (f.form_id, Some(User::parse(f.bot_id, &entities)), Some(f.title), Some(f.description), f.invoice)
            }
            base::payments::PaymentForm::PaymentFormStars(f) => {
                let f = *f;
                let entities = Entities::new(client, f.users, Vec::new());
                (f.form_id, Some(User::parse(f.bot_id, &entities)), Some(f.title), Some(f.description), f.invoice)
            }
            base::payments::PaymentForm::PaymentFormStarGift(f) => (f.form_id, None, None, None, f.invoice),
        };
        let base::Invoice::Invoice(terms) = terms;
        let prices: Vec<LabeledPrice> = terms
            .prices
            .into_iter()
            .map(|base::LabeledPrice::LabeledPrice(p)| LabeledPrice { label: p.label, amount: p.amount })
            .collect();
        PaymentForm {
            id,
            bot,
            title,
            description,
            currency: terms.currency,
            // A hostile price list must not overflow into a small total.
            total_amount: prices.iter().map(|p| p.amount).fold(0, i64::saturating_add),
            prices,
            subscription_period: terms.subscription_period,
            is_test: terms.test,
            invoice,
        }
    }
}
