use crate::Client;
use crate::errors::{Error, Result};
use crate::methods::messages::geo_point;
use crate::raw::functions;

impl Client {
    /// The address is at most 96 characters.
    pub async fn set_business_location(
        &self,
        address: impl Into<String>,
        location: Option<(f64, f64)>,
    ) -> Result<bool> {
        let address = address.into();
        if address.is_empty() || address.chars().count() > 96 {
            return Err(Error::InvalidArgument("a business address takes 1 to 96 characters"));
        }
        let on_earth = |(lat, long): (f64, f64)| (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&long);
        if location.is_some_and(|l| !on_earth(l)) {
            return Err(Error::InvalidArgument("latitude must be within 90 and longitude within 180 degrees"));
        }
        let geo_point = location.map(|(lat, long)| geo_point(lat, long, None));
        self.invoke(&functions::account::UpdateBusinessLocation { geo_point, address: Some(address) }).await
    }
}
