use crate::raw::{base, types};

/// A delivery address entered by the buyer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShippingAddress {
    /// Two-letter ISO 3166-1 country code.
    pub country_code: String,
    /// State, if applicable.
    pub state: String,
    /// City.
    pub city: String,
    /// First line of the street address.
    pub street_line1: String,
    /// Second line of the street address.
    pub street_line2: String,
    /// Postal code.
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

    pub(crate) fn write(self) -> base::PostAddress {
        base::PostAddress::PostAddress(Box::new(types::PostAddress {
            street_line1: self.street_line1,
            street_line2: self.street_line2,
            city: self.city,
            state: self.state,
            country_iso2: self.country_code,
            post_code: self.post_code,
        }))
    }
}
