use crate::raw::base;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShippingAddress {
    pub country_code: String,
    pub state: String,
    pub city: String,
    pub street_line1: String,
    pub street_line2: String,
    pub post_code: String,
}

impl ShippingAddress {
    pub(crate) fn parse(raw: &base::PostAddress) -> ShippingAddress {
        let base::PostAddress::PostAddress(a) = raw;
        ShippingAddress {
            country_code: a.country_iso2.clone(),
            state: a.state.clone(),
            city: a.city.clone(),
            street_line1: a.street_line1.clone(),
            street_line2: a.street_line2.clone(),
            post_code: a.post_code.clone(),
        }
    }
}
