//! Payments: invoices and their links, shipping and pre-checkout answers, the bot's Stars (balance, transactions,
//! refunds, subscriptions), a business account's Star balance, and Passport errors.

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::{done, no_updates, parse};
use crate::bot_api::bridge::{Transport, options, reply_parameters, unsupported};
use crate::bot_api::convert::{self, Peers, int};
use crate::bot_api::raw::types as bot;
use crate::errors::{Error, Result};
use crate::raw::{Deserializable, Identifiable, Serializable, base, functions as f, types};
use crate::utils::{base64_encode, get_peer};

const SEND_MEDIA: u32 = <f::messages::SendMedia as Identifiable>::CONSTRUCTOR_ID;
const EXPORT_INVOICE: u32 = <f::payments::ExportInvoice as Identifiable>::CONSTRUCTOR_ID;
const SHIPPING: u32 = <f::messages::SetBotShippingResults as Identifiable>::CONSTRUCTOR_ID;
const PRECHECKOUT: u32 = <f::messages::SetBotPrecheckoutResults as Identifiable>::CONSTRUCTOR_ID;
const STARS_STATUS: u32 = <f::payments::GetStarsStatus as Identifiable>::CONSTRUCTOR_ID;
const STARS_TRANSACTIONS: u32 = <f::payments::GetStarsTransactions as Identifiable>::CONSTRUCTOR_ID;
const REFUND: u32 = <f::payments::RefundStarsCharge as Identifiable>::CONSTRUCTOR_ID;
const CANCEL_SUBSCRIPTION: u32 = <f::payments::BotCancelStarsSubscription as Identifiable>::CONSTRUCTOR_ID;
const PASSPORT_ERRORS: u32 = <f::users::SetSecureValueErrors as Identifiable>::CONSTRUCTOR_ID;

/// A Bot API `StarAmount`.
#[derive(Deserialize)]
struct StarAmount {
    #[serde(default)]
    amount: i64,
    #[serde(default)]
    nanostar_amount: i64,
}

impl Transport {
    /// Answers a payments request, made through the business connection `connection` if set; `None` when it isn't
    /// of this group.
    pub(super) async fn answer_payments(
        &self,
        id: u32,
        body: &[u8],
        connection: Option<&str>,
    ) -> Option<Result<Vec<u8>>> {
        Some(match (id, connection) {
            // Other media are the messages group's: only an invoice is a payment.
            (SEND_MEDIA, _) => {
                let r = match f::messages::SendMedia::from_bytes(body) {
                    Ok(r) => r,
                    Err(e) => return Some(Err(e.into())),
                };
                if !matches!(r.media, base::InputMedia::InputMediaInvoice(_)) {
                    return None;
                }
                match connection {
                    Some(_) => Err(unsupported("sending an invoice through a business connection")),
                    None => self.send_invoice(r).await,
                }
            }
            (EXPORT_INVOICE, _) => self.create_invoice_link(body, connection).await,
            (STARS_STATUS, _) => self.star_balance(body, connection).await,
            (SHIPPING, None) => self.answer_shipping_query(body).await,
            (PRECHECKOUT, None) => self.answer_pre_checkout_query(body).await,
            (STARS_TRANSACTIONS, None) => self.star_transactions(body).await,
            (REFUND, None) => self.refund_star_payment(body).await,
            (CANCEL_SUBSCRIPTION, None) => self.edit_star_subscription(body).await,
            (PASSPORT_ERRORS, None) => self.set_passport_data_errors(body).await,
            _ => return None,
        })
    }

    /// `send_invoice`: TL sends an invoice as a media message.
    async fn send_invoice(&self, r: f::messages::SendMedia) -> Result<Vec<u8>> {
        if r.schedule_date.is_some() || r.send_as.is_some() || r.quick_reply_shortcut.is_some() {
            return Err(unsupported("scheduled invoices, or invoices sent as another chat or to quick replies"));
        }
        if r.allow_paid_stars.is_some() || r.suggested_post.is_some() {
            return Err(unsupported("paid messages and suggested posts"));
        }
        let me = self.me();
        let mut body = Map::new();
        body.insert("chat_id".into(), self.chat_id(&r.peer)?.into());
        options(&mut body, r.silent, r.noforwards, r.allow_paid_floodskip);
        if let Some(reply) = &r.reply_to {
            reply_parameters(&mut body, reply, me)?;
        }
        if let Some(markup) = &r.reply_markup {
            body.insert("reply_markup".into(), convert::markup_json(markup, me)?);
        }
        if let Some(effect) = r.effect {
            body.insert("message_effect_id".into(), effect.to_string().into());
        }
        invoice_json(&r.media, &mut body, true)?;
        let sent = self.call("sendInvoice", Value::Object(body)).await?;
        self.sent(vec![sent], false)
    }

    async fn create_invoice_link(&self, body: &[u8], connection: Option<&str>) -> Result<Vec<u8>> {
        let r = f::payments::ExportInvoice::from_bytes(body)?;
        let mut out = Map::new();
        if let Some(connection) = connection {
            out.insert("business_connection_id".into(), connection.into());
        }
        invoice_json(&r.invoice_media, &mut out, false)?;
        let url: String = parse(self.call("createInvoiceLink", Value::Object(out)).await?)?;
        Ok(types::payments::ExportedInvoice { url }.to_bytes())
    }

    async fn answer_shipping_query(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::messages::SetBotShippingResults::from_bytes(body)?;
        let mut out = Map::new();
        out.insert("shipping_query_id".into(), r.query_id.to_string().into());
        out.insert("ok".into(), r.error.is_none().into());
        if let Some(error) = r.error {
            out.insert("error_message".into(), error.into());
        }
        if let Some(list) = &r.shipping_options {
            let list: Vec<Value> = list
                .iter()
                .map(|base::ShippingOption::ShippingOption(o)| {
                    json!({ "id": o.id, "title": o.title, "prices": prices_json(&o.prices) })
                })
                .collect();
            out.insert("shipping_options".into(), list.into());
        }
        self.call("answerShippingQuery", Value::Object(out)).await?;
        Ok(done())
    }

    async fn answer_pre_checkout_query(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::messages::SetBotPrecheckoutResults::from_bytes(body)?;
        let mut out = Map::new();
        out.insert("pre_checkout_query_id".into(), r.query_id.to_string().into());
        out.insert("ok".into(), r.success.into());
        if let Some(error) = r.error {
            out.insert("error_message".into(), error.into());
        }
        self.call("answerPreCheckoutQuery", Value::Object(out)).await?;
        Ok(done())
    }

    /// The bot's own balance, or the balance of the business account behind `connection`.
    async fn star_balance(&self, body: &[u8], connection: Option<&str>) -> Result<Vec<u8>> {
        let r = f::payments::GetStarsStatus::from_bytes(body)?;
        if r.ton {
            return Err(unsupported("TON balances"));
        }
        if !matches!(r.peer, base::InputPeer::InputPeerSelf) {
            return Err(unsupported("the Star balance of another chat"));
        }
        let balance = match connection {
            Some(connection) => {
                self.call("getBusinessAccountStarBalance", json!({ "business_connection_id": connection })).await?
            }
            None => self.call("getMyStarBalance", json!({})).await?,
        };
        let balance: StarAmount = parse(balance)?;
        Ok(stars_status(&balance, None, None).to_bytes())
    }

    /// The bot's transactions, newest first. The Bot API pages by a count of transactions, which is the offset
    /// string here; inbound and outbound are picked out of each page.
    async fn star_transactions(&self, body: &[u8]) -> Result<Vec<u8>> {
        #[derive(Deserialize)]
        struct Partner {
            #[serde(rename = "type")]
            kind: String,
            user: Option<bot::User>,
            chat: Option<bot::Chat>,
            invoice_payload: Option<String>,
            subscription_period: Option<i64>,
        }
        #[derive(Deserialize)]
        struct Transaction {
            id: String,
            #[serde(default)]
            amount: i64,
            #[serde(default)]
            nanostar_amount: i64,
            #[serde(default)]
            date: i64,
            source: Option<Partner>,
            receiver: Option<Partner>,
        }
        #[derive(Deserialize)]
        struct Transactions {
            #[serde(default)]
            transactions: Vec<Transaction>,
        }
        let r = f::payments::GetStarsTransactions::from_bytes(body)?;
        if !matches!(r.peer, base::InputPeer::InputPeerSelf) || r.subscription_id.is_some() {
            return Err(unsupported("the Star transactions of another chat or of one subscription"));
        }
        if r.ton || r.ascending {
            return Err(unsupported("TON transactions, or transactions oldest first"));
        }
        let offset: i64 = match r.offset.as_str() {
            "" => 0,
            offset => {
                offset.parse().map_err(|_| Error::InvalidArgument("this offset isn't from a client in Bot API mode"))?
            }
        };
        let body = json!({ "offset": offset, "limit": r.limit });
        let page: Transactions = parse(self.call("getStarTransactions", body).await?)?;
        let read = page.transactions.len();
        let mut peers = Peers::new(self.me());
        let mut history = Vec::with_capacity(read);
        for t in page.transactions {
            let incoming = t.receiver.is_none();
            if (r.inbound && !incoming) || (r.outbound && incoming) {
                continue;
            }
            let partner = if incoming { t.source } else { t.receiver };
            let sign = if incoming { 1 } else { -1 };
            let amount = star_amount(sign * t.amount, sign * t.nanostar_amount);
            let peer = match partner.as_ref().map(|p| p.kind.as_str()) {
                Some("user") => match partner.as_ref().and_then(|p| p.user.as_ref()) {
                    Some(user) => {
                        peers.user(user);
                        transaction_peer(get_peer(user.id))
                    }
                    None => base::StarsTransactionPeer::StarsTransactionPeerUnsupported,
                },
                Some("chat") => match partner.as_ref().and_then(|p| p.chat.as_ref()) {
                    Some(chat) => {
                        peers.chat(chat);
                        transaction_peer(get_peer(chat.id))
                    }
                    None => base::StarsTransactionPeer::StarsTransactionPeerUnsupported,
                },
                Some("fragment") => base::StarsTransactionPeer::StarsTransactionPeerFragment,
                Some("telegram_ads") => base::StarsTransactionPeer::StarsTransactionPeerAds,
                Some("telegram_api") => base::StarsTransactionPeer::StarsTransactionPeerAPI,
                _ => base::StarsTransactionPeer::StarsTransactionPeerUnsupported,
            };
            let mut transaction = types::StarsTransaction::new(t.id, amount, int(t.date), peer);
            if let Some(partner) = &partner {
                transaction.bot_payload = partner.invoice_payload.as_ref().map(|p| p.clone().into_bytes());
                transaction.subscription_period = partner.subscription_period.map(int);
            }
            history.push(base::StarsTransaction::StarsTransaction(Box::new(transaction)));
        }
        let next = (read > 0 && read as i64 >= i64::from(r.limit)).then(|| (offset + read as i64).to_string());
        let mut status = stars_status(&StarAmount { amount: 0, nanostar_amount: 0 }, Some(history), next);
        status.users = peers.users;
        status.chats = peers.chats;
        Ok(status.to_bytes())
    }

    async fn refund_star_payment(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::payments::RefundStarsCharge::from_bytes(body)?;
        let body = json!({ "user_id": self.user_id(&r.user_id)?, "telegram_payment_charge_id": r.charge_id });
        self.call("refundStarPayment", body).await?;
        Ok(no_updates())
    }

    async fn edit_star_subscription(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::payments::BotCancelStarsSubscription::from_bytes(body)?;
        let body = json!({
            "user_id": self.user_id(&r.user_id)?,
            "telegram_payment_charge_id": r.charge_id,
            "is_canceled": !r.restore,
        });
        self.call("editUserStarSubscription", body).await?;
        Ok(done())
    }

    async fn set_passport_data_errors(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::users::SetSecureValueErrors::from_bytes(body)?;
        let errors: Vec<Value> = r.errors.iter().map(passport_error_json).collect();
        self.call("setPassportDataErrors", json!({ "user_id": self.user_id(&r.id)?, "errors": errors })).await?;
        Ok(done())
    }
}

/// TL's Stars status holding a balance, and a page of transactions if one was read.
fn stars_status(
    balance: &StarAmount,
    history: Option<Vec<base::StarsTransaction>>,
    next_offset: Option<String>,
) -> types::payments::StarsStatus {
    let balance = star_amount(balance.amount, balance.nanostar_amount);
    let mut status = types::payments::StarsStatus::new(balance, Vec::new(), Vec::new());
    status.history = history;
    status.next_offset = next_offset;
    status
}

fn star_amount(amount: i64, nanos: i64) -> base::StarsAmount {
    base::StarsAmount::StarsAmount(Box::new(types::StarsAmount { amount, nanos: int(nanos) }))
}

fn transaction_peer(peer: base::Peer) -> base::StarsTransactionPeer {
    base::StarsTransactionPeer::StarsTransactionPeer(Box::new(types::StarsTransactionPeer { peer }))
}

fn prices_json(prices: &[base::LabeledPrice]) -> Value {
    let prices: Vec<Value> = prices
        .iter()
        .map(|base::LabeledPrice::LabeledPrice(p)| json!({ "label": p.label, "amount": p.amount }))
        .collect();
    Value::Array(prices)
}

/// The fields of an invoice that `sendInvoice` and `createInvoiceLink` share, put into `out`; `start_parameter` only
/// with `sent`, as a link has none.
fn invoice_json(media: &base::InputMedia, out: &mut Map<String, Value>, sent: bool) -> Result<()> {
    let base::InputMedia::InputMediaInvoice(m) = media else {
        return Err(Error::InvalidArgument("an invoice request holds no invoice"));
    };
    if m.extended_media.is_some() {
        return Err(unsupported("invoices with paid media"));
    }
    let base::Invoice::Invoice(invoice) = &m.invoice;
    out.insert("title".into(), m.title.clone().into());
    out.insert("description".into(), m.description.clone().into());
    // The Bot API's payload is a string, and a lossy one would hand the bot back bytes it never sent.
    let payload = std::str::from_utf8(&m.payload)
        .map_err(|_| Error::InvalidArgument("the Bot API only takes an invoice payload that is valid UTF-8"))?;
    out.insert("payload".into(), payload.into());
    if let Some(provider) = m.provider.as_ref().filter(|p| !p.is_empty()) {
        out.insert("provider_token".into(), provider.clone().into());
    }
    out.insert("currency".into(), invoice.currency.clone().into());
    out.insert("prices".into(), prices_json(&invoice.prices));
    if let Some(max) = invoice.max_tip_amount {
        out.insert("max_tip_amount".into(), max.into());
    }
    if let Some(tips) = &invoice.suggested_tip_amounts {
        out.insert("suggested_tip_amounts".into(), tips.clone().into());
    }
    if let Some(period) = invoice.subscription_period {
        out.insert("subscription_period".into(), period.into());
    }
    let base::DataJSON::DataJSON(data) = &m.provider_data;
    if !matches!(data.data.trim(), "" | "{}") {
        out.insert("provider_data".into(), data.data.clone().into());
    }
    if let Some(base::InputWebDocument::InputWebDocument(photo)) = &m.photo {
        out.insert("photo_url".into(), photo.url.clone().into());
        if photo.size > 0 {
            out.insert("photo_size".into(), photo.size.into());
        }
        for attribute in &photo.attributes {
            if let base::DocumentAttribute::DocumentAttributeImageSize(size) = attribute {
                out.insert("photo_width".into(), size.w.into());
                out.insert("photo_height".into(), size.h.into());
            }
        }
    }
    let flags = [
        ("need_name", invoice.name_requested),
        ("need_phone_number", invoice.phone_requested),
        ("need_email", invoice.email_requested),
        ("need_shipping_address", invoice.shipping_address_requested),
        ("send_phone_number_to_provider", invoice.phone_to_provider),
        ("send_email_to_provider", invoice.email_to_provider),
        ("is_flexible", invoice.flexible),
    ];
    for (key, _) in flags.into_iter().filter(|&(_, on)| on) {
        out.insert(key.into(), true.into());
    }
    if let Some(start) = m.start_param.as_ref().filter(|_| sent) {
        out.insert("start_parameter".into(), start.clone().into());
    }
    Ok(())
}

/// A TL Passport error as the Bot API's `PassportElementError`; hashes go as base64.
fn passport_error_json(error: &base::SecureValueError) -> Value {
    use base::SecureValueError as E;
    let (source, kind, mut out) = match error {
        E::SecureValueErrorData(e) => {
            let fields = json!({ "field_name": e.field, "data_hash": base64_encode(&e.data_hash), "message": e.text });
            ("data", &e.r#type, fields)
        }
        E::SecureValueErrorFrontSide(e) => ("front_side", &e.r#type, file_error(&e.file_hash, &e.text)),
        E::SecureValueErrorReverseSide(e) => ("reverse_side", &e.r#type, file_error(&e.file_hash, &e.text)),
        E::SecureValueErrorSelfie(e) => ("selfie", &e.r#type, file_error(&e.file_hash, &e.text)),
        E::SecureValueErrorFile(e) => ("file", &e.r#type, file_error(&e.file_hash, &e.text)),
        E::SecureValueErrorFiles(e) => ("files", &e.r#type, files_error(&e.file_hash, &e.text)),
        E::SecureValueErrorTranslationFile(e) => ("translation_file", &e.r#type, file_error(&e.file_hash, &e.text)),
        E::SecureValueErrorTranslationFiles(e) => ("translation_files", &e.r#type, files_error(&e.file_hash, &e.text)),
        E::SecureValueError(e) => {
            ("unspecified", &e.r#type, json!({ "element_hash": base64_encode(&e.hash), "message": e.text }))
        }
    };
    if let Value::Object(fields) = &mut out {
        fields.insert("source".into(), source.into());
        fields.insert("type".into(), secure_value_type(kind).into());
    }
    out
}

fn file_error(hash: &[u8], text: &str) -> Value {
    json!({ "file_hash": base64_encode(hash), "message": text })
}

fn files_error(hashes: &[Vec<u8>], text: &str) -> Value {
    let hashes: Vec<String> = hashes.iter().map(|hash| base64_encode(hash)).collect();
    json!({ "file_hashes": hashes, "message": text })
}

fn secure_value_type(kind: &base::SecureValueType) -> &'static str {
    use base::SecureValueType as T;
    match kind {
        T::SecureValueTypePersonalDetails => "personal_details",
        T::SecureValueTypePassport => "passport",
        T::SecureValueTypeDriverLicense => "driver_license",
        T::SecureValueTypeIdentityCard => "identity_card",
        T::SecureValueTypeInternalPassport => "internal_passport",
        T::SecureValueTypeAddress => "address",
        T::SecureValueTypeUtilityBill => "utility_bill",
        T::SecureValueTypeBankStatement => "bank_statement",
        T::SecureValueTypeRentalAgreement => "rental_agreement",
        T::SecureValueTypePassportRegistration => "passport_registration",
        T::SecureValueTypeTemporaryRegistration => "temporary_registration",
        T::SecureValueTypePhone => "phone_number",
        T::SecureValueTypeEmail => "email",
    }
}

#[cfg(test)]
mod tests {
    use super::{invoice_json, passport_error_json};
    use crate::errors::Error;
    use crate::raw::{base, types};

    fn invoice(payload: Vec<u8>) -> base::InputMedia {
        let invoice = base::Invoice::Invoice(Box::new(types::Invoice::new("XTR".into(), Vec::new())));
        let data = base::DataJSON::DataJSON(Box::new(types::DataJSON { data: "{}".into() }));
        let media = types::InputMediaInvoice::new("Pack".into(), "Stickers".into(), invoice, payload, data);
        base::InputMedia::InputMediaInvoice(Box::new(media))
    }

    #[test]
    fn an_invoice_payload_goes_as_its_text_and_other_bytes_are_refused() {
        let mut out = serde_json::Map::new();
        invoice_json(&invoice(b"order-1".to_vec()), &mut out, false).unwrap();
        assert_eq!(out["payload"], "order-1");
        let error = invoice_json(&invoice(vec![0xff, 0x00]), &mut serde_json::Map::new(), false).unwrap_err();
        assert!(matches!(error, Error::InvalidArgument(_)), "{error:?}");
    }

    #[test]
    fn passport_errors_name_their_source_and_type() {
        let error = types::SecureValueErrorFiles {
            r#type: base::SecureValueType::SecureValueTypeUtilityBill,
            file_hash: vec![vec![1, 2, 3], vec![255]],
            text: "blurry".into(),
        };
        let json = passport_error_json(&base::SecureValueError::SecureValueErrorFiles(Box::new(error)));
        assert_eq!(
            json,
            serde_json::json!({
                "source": "files",
                "type": "utility_bill",
                "file_hashes": ["AQID", "/w=="],
                "message": "blurry",
            })
        );
    }
}
