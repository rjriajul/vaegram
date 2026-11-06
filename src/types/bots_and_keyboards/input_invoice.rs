use crate::raw::{base, types};
use crate::types::LabeledPrice;

/// An invoice to send in a message or as an inline result.
#[derive(Debug, Clone, PartialEq)]
pub struct InputInvoice(types::InputMediaInvoice);

impl InputInvoice {
    /// Creates an invoice with no provider, photo, tips or requested user details.
    pub fn new(
        title: impl Into<String>,
        description: impl Into<String>,
        payload: impl Into<String>,
        currency: impl Into<String>,
        prices: impl IntoIterator<Item = LabeledPrice>,
    ) -> InputInvoice {
        let invoice = types::Invoice {
            test: false,
            name_requested: false,
            phone_requested: false,
            email_requested: false,
            shipping_address_requested: false,
            flexible: false,
            phone_to_provider: false,
            email_to_provider: false,
            recurring: false,
            currency: currency.into(),
            prices: prices.into_iter().map(LabeledPrice::write).collect(),
            max_tip_amount: None,
            suggested_tip_amounts: None,
            terms_url: None,
            subscription_period: None,
        };
        let media = types::InputMediaInvoice {
            title: title.into(),
            description: description.into(),
            photo: None,
            invoice: base::Invoice::Invoice(Box::new(invoice)),
            payload: payload.into().into_bytes(),
            provider: None,
            provider_data: base::DataJSON::DataJSON(Box::new(types::DataJSON { data: "{}".into() })),
            start_param: None,
            extended_media: None,
        };
        InputInvoice(media)
    }

    fn invoice(&mut self) -> &mut types::Invoice {
        let base::Invoice::Invoice(invoice) = &mut self.0.invoice;
        invoice
    }

    /// Sends the payment through the provider this token belongs to.
    #[must_use]
    pub fn provider_token(mut self, token: impl Into<String>) -> Self {
        self.0.provider = Some(token.into());
        self
    }

    /// JSON data passed to the payment provider. Defaults to `{}`.
    #[must_use]
    pub fn provider_data(mut self, json: impl Into<String>) -> Self {
        self.0.provider_data = base::DataJSON::DataJSON(Box::new(types::DataJSON { data: json.into() }));
        self
    }

    /// Adds a start parameter for deep links to the bot.
    #[must_use]
    pub fn start_parameter(mut self, parameter: impl Into<String>) -> Self {
        self.0.start_param = Some(parameter.into());
        self
    }

    /// Shows the photo at `url` on the invoice. The size is sent only when `width` and `height` are both positive.
    #[must_use]
    pub fn photo(mut self, url: impl Into<String>, width: i32, height: i32) -> Self {
        let size = types::DocumentAttributeImageSize { w: width, h: height };
        let attributes = if width > 0 && height > 0 {
            vec![base::DocumentAttribute::DocumentAttributeImageSize(size)]
        } else {
            Vec::new()
        };
        let photo = types::InputWebDocument { url: url.into(), size: 0, mime_type: "image/jpeg".into(), attributes };
        self.0.photo = Some(base::InputWebDocument::InputWebDocument(Box::new(photo)));
        self
    }

    /// Lets the buyer tip up to `max_tip_amount`, offering the suggested amounts as presets. Amounts are in the
    /// currency's smallest units.
    #[must_use]
    pub fn tips(mut self, max_tip_amount: i64, suggested_tip_amounts: impl IntoIterator<Item = i64>) -> Self {
        let invoice = self.invoice();
        invoice.max_tip_amount = Some(max_tip_amount);
        invoice.suggested_tip_amounts = Some(suggested_tip_amounts.into_iter().collect());
        self
    }

    /// Telegram only accepts 2592000 (30 days), and only for invoice links in Telegram Stars.
    #[must_use]
    pub fn subscription_period(mut self, seconds: i32) -> Self {
        let invoice = self.invoice();
        (invoice.recurring, invoice.subscription_period) = (true, Some(seconds));
        self
    }

    /// Asks the user for their full name.
    #[must_use]
    pub fn need_name(mut self) -> Self {
        self.invoice().name_requested = true;
        self
    }

    /// Asks the user for their phone number.
    #[must_use]
    pub fn need_phone_number(mut self) -> Self {
        self.invoice().phone_requested = true;
        self
    }

    /// Asks the user for their email address.
    #[must_use]
    pub fn need_email(mut self) -> Self {
        self.invoice().email_requested = true;
        self
    }

    /// Asks the user for a shipping address.
    #[must_use]
    pub fn need_shipping_address(mut self) -> Self {
        self.invoice().shipping_address_requested = true;
        self
    }

    /// Passes the user's phone number to the payment provider.
    #[must_use]
    pub fn send_phone_number_to_provider(mut self) -> Self {
        self.invoice().phone_to_provider = true;
        self
    }

    /// Passes the user's email address to the payment provider.
    #[must_use]
    pub fn send_email_to_provider(mut self) -> Self {
        self.invoice().email_to_provider = true;
        self
    }

    /// Makes the final price depend on the shipping method.
    #[must_use]
    pub fn flexible(mut self) -> Self {
        self.invoice().flexible = true;
        self
    }

    pub(crate) fn write(self) -> base::InputMedia {
        base::InputMedia::InputMediaInvoice(Box::new(self.0))
    }

    pub(crate) fn write_inline(self, reply_markup: Option<base::ReplyMarkup>) -> base::InputBotInlineMessage {
        let m = self.0;
        let invoice = types::InputBotInlineMessageMediaInvoice {
            title: m.title,
            description: m.description,
            photo: m.photo,
            invoice: m.invoice,
            payload: m.payload,
            provider: m.provider.unwrap_or_default(),
            provider_data: m.provider_data,
            reply_markup,
        };
        base::InputBotInlineMessage::InputBotInlineMessageMediaInvoice(Box::new(invoice))
    }
}
