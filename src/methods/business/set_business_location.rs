use crate::Client;
use crate::errors::{Error, Result};
use crate::methods::accepted;
use crate::methods::messages::geo_point;
use crate::raw::functions;

/// Request built by [`Client::set_business_location`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SetBusinessLocation<'a> {
    client: &'a Client,
    address: String,
    location: Option<(f64, f64)>,
}

impl Client {
    /// Sets the address of the business account.
    ///
    /// The address takes 1 to 96 characters; anything else fails with `Error::InvalidArgument`. Without
    /// [`location`](SetBusinessLocation::location) the address has no point on the map. Awaiting the request returns
    /// `()`.
    pub fn set_business_location(&self, address: impl Into<String>) -> SetBusinessLocation<'_> {
        SetBusinessLocation { client: self, address: address.into(), location: None }
    }
}

impl SetBusinessLocation<'_> {
    /// Places the address at this point on the map. A latitude beyond 90 or a longitude beyond 180 degrees fails
    /// with `Error::InvalidArgument`.
    pub fn location(mut self, latitude: f64, longitude: f64) -> Self {
        self.location = Some((latitude, longitude));
        self
    }

    async fn send(self) -> Result<()> {
        let address = self.address;
        if address.is_empty() || address.chars().count() > 96 {
            return Err(Error::InvalidArgument("a business address takes 1 to 96 characters"));
        }
        let on_earth = |(lat, long): (f64, f64)| (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&long);
        if self.location.is_some_and(|l| !on_earth(l)) {
            return Err(Error::InvalidArgument("latitude must be within 90 and longitude within 180 degrees"));
        }
        let geo_point = self.location.map(|(lat, long)| geo_point(lat, long, None));
        let request = functions::account::UpdateBusinessLocation { geo_point, address: Some(address) };
        accepted(self.client.invoke(&request).await?)
    }
}

into_future!(SetBusinessLocation, ());
