use crate::raw::{base, types};
use crate::types::ShippingAddress;

/// Contact and delivery details the buyer filled in during checkout.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderInfo {
    /// The buyer's name. `None` if not provided.
    pub name: Option<String>,
    /// The buyer's phone number. `None` if not provided.
    pub phone_number: Option<String>,
    /// The buyer's email address. `None` if not provided.
    pub email: Option<String>,
    /// Where to deliver. `None` if not provided.
    pub shipping_address: Option<ShippingAddress>,
}

impl OrderInfo {
    /// Provides nothing, like `Default`; fill in what the invoice asks for with the setters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the buyer's name.
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the buyer's phone number.
    #[must_use]
    pub fn phone_number(mut self, phone_number: impl Into<String>) -> Self {
        self.phone_number = Some(phone_number.into());
        self
    }

    /// Sets the buyer's email address.
    #[must_use]
    pub fn email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }

    /// Sets where to deliver.
    #[must_use]
    pub fn shipping_address(mut self, shipping_address: ShippingAddress) -> Self {
        self.shipping_address = Some(shipping_address);
        self
    }

    pub(crate) fn parse(raw: &base::PaymentRequestedInfo) -> OrderInfo {
        let base::PaymentRequestedInfo::PaymentRequestedInfo(i) = raw;
        OrderInfo {
            name: i.name.clone(),
            phone_number: i.phone.clone(),
            email: i.email.clone(),
            shipping_address: i.shipping_address.as_ref().map(ShippingAddress::parse),
        }
    }

    pub(crate) fn write(self) -> base::PaymentRequestedInfo {
        base::PaymentRequestedInfo::PaymentRequestedInfo(Box::new(types::PaymentRequestedInfo {
            name: self.name,
            phone: self.phone_number,
            email: self.email,
            shipping_address: self.shipping_address.map(ShippingAddress::write),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::OrderInfo;
    use crate::raw::base;
    use crate::types::ShippingAddress;

    #[test]
    fn order_info_round_trips() {
        let info = OrderInfo {
            name: Some("Ann".into()),
            phone_number: Some("+100".into()),
            email: None,
            shipping_address: Some(ShippingAddress::new("US", "LA", "1 Main", "90001").state("CA")),
        };
        assert_eq!(OrderInfo::parse(&info.clone().write()), info);
        let empty = OrderInfo { name: None, phone_number: None, email: None, shipping_address: None };
        assert_eq!(OrderInfo::parse(&empty.clone().write()), empty);
    }

    #[test]
    fn builds_from_new_and_setters() {
        let base::PaymentRequestedInfo::PaymentRequestedInfo(empty) = OrderInfo::new().write();
        assert!(
            empty.name.is_none() && empty.phone.is_none() && empty.email.is_none() && empty.shipping_address.is_none()
        );
        let info =
            OrderInfo::new().email("a@b.c").shipping_address(ShippingAddress::new("US", "LA", "1 Main", "90001"));
        let base::PaymentRequestedInfo::PaymentRequestedInfo(raw) = info.write();
        assert_eq!((raw.name, raw.email.as_deref()), (None, Some("a@b.c")));
        let Some(base::PostAddress::PostAddress(address)) = raw.shipping_address else { panic!("no address written") };
        assert_eq!((address.country_iso2.as_str(), address.post_code.as_str()), ("US", "90001"));
    }
}
