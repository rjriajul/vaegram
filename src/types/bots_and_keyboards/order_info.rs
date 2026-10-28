use crate::raw::base;
use crate::types::ShippingAddress;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderInfo {
    pub name: Option<String>,
    pub phone_number: Option<String>,
    pub email: Option<String>,
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
}
