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
    /// Creates an address with an empty state and second street line.
    pub fn new(
        country_code: impl Into<String>,
        city: impl Into<String>,
        street_line1: impl Into<String>,
        post_code: impl Into<String>,
    ) -> Self {
        ShippingAddress {
            country_code: country_code.into(),
            state: String::new(),
            city: city.into(),
            street_line1: street_line1.into(),
            street_line2: String::new(),
            post_code: post_code.into(),
        }
    }

    /// Sets the state, for countries that have them.
    #[must_use]
    pub fn state(mut self, state: impl Into<String>) -> Self {
        self.state = state.into();
        self
    }

    /// Sets the second line of the street address.
    #[must_use]
    pub fn street_line2(mut self, street_line2: impl Into<String>) -> Self {
        self.street_line2 = street_line2.into();
        self
    }

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

#[cfg(test)]
mod tests {
    use super::ShippingAddress;
    use crate::raw::base;

    #[test]
    fn new_leaves_state_and_second_line_empty_until_set() {
        let base::PostAddress::PostAddress(plain) = ShippingAddress::new("DE", "Berlin", "Main St 1", "10115").write();
        assert_eq!(
            (plain.country_iso2.as_str(), plain.city.as_str(), plain.street_line1.as_str(), plain.post_code.as_str()),
            ("DE", "Berlin", "Main St 1", "10115")
        );
        assert!(plain.state.is_empty() && plain.street_line2.is_empty());
        let full = ShippingAddress::new("US", "LA", "1 Main", "90001").state("CA").street_line2("Apt 2");
        let base::PostAddress::PostAddress(raw) = full.write();
        assert_eq!((raw.state.as_str(), raw.street_line2.as_str()), ("CA", "Apt 2"));
    }
}
