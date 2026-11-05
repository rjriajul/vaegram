use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::PaymentFormType;
use crate::raw::base;
use crate::types::{LabeledPrice, OrderInfo, PaymentOption, SavedCredentials, User};

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
#[derive(Clone, PartialEq)]
pub struct PaymentForm {
    /// The form id Telegram assigned.
    pub id: i64,
    /// Which kind of payment this is, derived from the form and its invoice.
    pub kind: PaymentFormType,
    /// The bot that sent the invoice. `None` for a Star gift form.
    pub bot: Option<User>,
    /// The invoice title. `None` for a Star gift form.
    pub title: Option<String>,
    /// The invoice description. `None` for a Star gift form.
    pub description: Option<String>,
    /// URL of the invoice photo. `None` if it has none.
    pub photo_url: Option<String>,
    /// Currency code of the prices.
    pub currency: String,
    /// The priced items.
    pub prices: Vec<LabeledPrice>,
    /// Sum of `prices` in the currency's smallest units. Saturates instead of overflowing.
    pub total_amount: i64,
    /// Seconds between charges: paying this form starts a subscription.
    pub subscription_period: Option<i32>,
    /// Whether the invoice is a test one.
    pub is_test: bool,
    /// The payment provider's account; only card forms have one.
    pub payment_provider: Option<User>,
    /// The provider's payment page; card forms only.
    pub url: Option<String>,
    pub native_provider: Option<String>,
    /// The provider's JSON parameters for paying in-app.
    pub native_params: Option<String>,
    /// Other ways to pay the form.
    pub additional_payment_options: Vec<PaymentOption>,
    /// Cards the provider remembers for this account.
    pub saved_credentials: Vec<SavedCredentials>,
    /// Order info saved by an earlier payment. `None` if there is none.
    pub saved_order_info: Option<OrderInfo>,
    /// Whether the provider can remember a new card.
    ///
    /// See [`InputCredentials::allow_save`](crate::types::InputCredentials::allow_save).
    pub can_save_credentials: bool,
    /// Paying with saved credentials needs a 2FA password, and this account has none.
    pub need_password: bool,
    /// Whether the invoice asks for the payer's name.
    pub is_name_requested: bool,
    /// Whether the invoice asks for the payer's phone number.
    pub is_phone_requested: bool,
    /// Whether the invoice asks for the payer's email address.
    pub is_email_requested: bool,
    /// Whether the invoice asks for a shipping address.
    pub is_shipping_address_requested: bool,
    /// Whether the final price depends on the shipping method.
    pub is_flexible: bool,
    /// Whether the phone number goes to the provider.
    pub is_phone_to_provider: bool,
    /// Whether the email address goes to the provider.
    pub is_email_to_provider: bool,
    /// Whether the invoice is for recurring payments.
    pub is_recurring: bool,
    /// Largest tip the payer can add, in the currency's smallest units. `None` if the invoice has no tips.
    pub max_tip_amount: Option<i64>,
    /// Tip amounts offered as shortcuts, in the currency's smallest units.
    pub suggested_tip_amounts: Vec<i64>,
    /// URL of the terms of service the payer agrees to. `None` if there are none.
    pub terms_url: Option<String>,
    pub(crate) invoice: base::InputInvoice,
}

// The saved order info and card titles are personal data, so a log of a form must not show them.
impl std::fmt::Debug for PaymentForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let redacted = |present: bool| if present { "<redacted>" } else { "None" };
        f.debug_struct("PaymentForm")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("bot", &self.bot)
            .field("title", &self.title)
            .field("description", &self.description)
            .field("photo_url", &self.photo_url)
            .field("currency", &self.currency)
            .field("prices", &self.prices)
            .field("total_amount", &self.total_amount)
            .field("subscription_period", &self.subscription_period)
            .field("is_test", &self.is_test)
            .field("payment_provider", &self.payment_provider)
            .field("url", &self.url)
            .field("native_provider", &self.native_provider)
            .field("native_params", &self.native_params)
            .field("additional_payment_options", &self.additional_payment_options)
            .field("saved_credentials", &format_args!("<{} redacted>", self.saved_credentials.len()))
            .field("saved_order_info", &format_args!("{}", redacted(self.saved_order_info.is_some())))
            .field("can_save_credentials", &self.can_save_credentials)
            .field("need_password", &self.need_password)
            .field("is_name_requested", &self.is_name_requested)
            .field("is_phone_requested", &self.is_phone_requested)
            .field("is_email_requested", &self.is_email_requested)
            .field("is_shipping_address_requested", &self.is_shipping_address_requested)
            .field("is_flexible", &self.is_flexible)
            .field("is_phone_to_provider", &self.is_phone_to_provider)
            .field("is_email_to_provider", &self.is_email_to_provider)
            .field("is_recurring", &self.is_recurring)
            .field("max_tip_amount", &self.max_tip_amount)
            .field("suggested_tip_amounts", &self.suggested_tip_amounts)
            .field("terms_url", &self.terms_url)
            .field("invoice", &self.invoice)
            .finish()
    }
}

fn web_document_url(raw: Option<&base::WebDocument>) -> Option<String> {
    match raw? {
        base::WebDocument::WebDocument(d) => Some(d.url.clone()),
        base::WebDocument::WebDocumentNoProxy(d) => Some(d.url.clone()),
    }
}

/// The parts of the three raw forms that differ.
struct Card {
    provider: Option<User>,
    url: Option<String>,
    native_provider: Option<String>,
    native_params: Option<String>,
    additional_payment_options: Vec<PaymentOption>,
    saved_credentials: Vec<SavedCredentials>,
    saved_order_info: Option<OrderInfo>,
    can_save_credentials: bool,
    need_password: bool,
}

impl PaymentForm {
    pub(crate) fn parse(client: &Client, raw: base::payments::PaymentForm, invoice: base::InputInvoice) -> PaymentForm {
        let mut card = Card {
            provider: None,
            url: None,
            native_provider: None,
            native_params: None,
            additional_payment_options: Vec::new(),
            saved_credentials: Vec::new(),
            saved_order_info: None,
            can_save_credentials: false,
            need_password: false,
        };
        let (id, bot, title, description, photo_url, terms) = match raw {
            base::payments::PaymentForm::PaymentForm(f) => {
                let f = *f;
                let entities = Entities::new(client, f.users, Vec::new());
                card = Card {
                    provider: Some(User::parse(f.provider_id, &entities)),
                    url: Some(f.url),
                    native_provider: f.native_provider,
                    native_params: f.native_params.map(|base::DataJSON::DataJSON(d)| d.data),
                    additional_payment_options: f
                        .additional_methods
                        .unwrap_or_default()
                        .into_iter()
                        .map(|base::PaymentFormMethod::PaymentFormMethod(m)| PaymentOption {
                            title: m.title,
                            url: m.url,
                        })
                        .collect(),
                    saved_credentials: f
                        .saved_credentials
                        .unwrap_or_default()
                        .into_iter()
                        .map(|base::PaymentSavedCredentials::PaymentSavedCredentialsCard(c)| SavedCredentials {
                            id: c.id,
                            title: c.title,
                        })
                        .collect(),
                    saved_order_info: f.saved_info.as_ref().map(OrderInfo::parse),
                    can_save_credentials: f.can_save_credentials,
                    need_password: f.password_missing,
                };
                (
                    f.form_id,
                    Some(User::parse(f.bot_id, &entities)),
                    Some(f.title),
                    Some(f.description),
                    web_document_url(f.photo.as_ref()),
                    f.invoice,
                )
            }
            base::payments::PaymentForm::PaymentFormStars(f) => {
                let f = *f;
                let entities = Entities::new(client, f.users, Vec::new());
                (
                    f.form_id,
                    Some(User::parse(f.bot_id, &entities)),
                    Some(f.title),
                    Some(f.description),
                    web_document_url(f.photo.as_ref()),
                    f.invoice,
                )
            }
            base::payments::PaymentForm::PaymentFormStarGift(f) => (f.form_id, None, None, None, None, f.invoice),
        };
        let is_card = card.provider.is_some();
        let is_gift = bot.is_none();
        let base::Invoice::Invoice(terms) = terms;
        let kind = match (is_card, is_gift, terms.subscription_period) {
            (true, _, _) => PaymentFormType::Regular,
            (_, true, _) => PaymentFormType::StarGift,
            (_, _, Some(_)) => PaymentFormType::StarSubscription,
            _ => PaymentFormType::Stars,
        };
        let prices: Vec<LabeledPrice> = terms
            .prices
            .into_iter()
            .map(|base::LabeledPrice::LabeledPrice(p)| LabeledPrice { label: p.label, amount: p.amount })
            .collect();
        PaymentForm {
            id,
            kind,
            bot,
            title,
            description,
            photo_url,
            currency: terms.currency,
            // A hostile price list must not overflow into a small total.
            total_amount: prices.iter().map(|p| p.amount).fold(0, i64::saturating_add),
            prices,
            subscription_period: terms.subscription_period,
            is_test: terms.test,
            payment_provider: card.provider,
            url: card.url,
            native_provider: card.native_provider,
            native_params: card.native_params,
            additional_payment_options: card.additional_payment_options,
            saved_credentials: card.saved_credentials,
            saved_order_info: card.saved_order_info,
            can_save_credentials: card.can_save_credentials,
            need_password: card.need_password,
            is_name_requested: terms.name_requested,
            is_phone_requested: terms.phone_requested,
            is_email_requested: terms.email_requested,
            is_shipping_address_requested: terms.shipping_address_requested,
            is_flexible: terms.flexible,
            is_phone_to_provider: terms.phone_to_provider,
            is_email_to_provider: terms.email_to_provider,
            is_recurring: terms.recurring,
            max_tip_amount: terms.max_tip_amount,
            suggested_tip_amounts: terms.suggested_tip_amounts.unwrap_or_default(),
            terms_url: terms.terms_url,
            invoice,
        }
    }
}
