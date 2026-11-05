use crate::raw::{base, types};
use crate::types::ShippingAddress;

/// Contact and delivery details the buyer filled in during checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    use crate::types::ShippingAddress;

    #[test]
    fn order_info_round_trips() {
        let info = OrderInfo {
            name: Some("Ann".into()),
            phone_number: Some("+100".into()),
            email: None,
            shipping_address: Some(ShippingAddress {
                country_code: "US".into(),
                state: "CA".into(),
                city: "LA".into(),
                street_line1: "1 Main".into(),
                street_line2: String::new(),
                post_code: "90001".into(),
            }),
        };
        assert_eq!(OrderInfo::parse(&info.clone().write()), info);
        let empty = OrderInfo { name: None, phone_number: None, email: None, shipping_address: None };
        assert_eq!(OrderInfo::parse(&empty.clone().write()), empty);
    }
}
