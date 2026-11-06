/// The data center closest to the client.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct NearestDc {
    /// Two-letter country code Telegram detected from the client's IP address.
    pub country: String,
    /// Id of the data center the client is connected to.
    pub this_dc: i32,
    /// Id of the data center closest to the client.
    pub nearest_dc: i32,
}
