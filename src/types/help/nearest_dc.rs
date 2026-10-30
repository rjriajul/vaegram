#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NearestDc {
    pub country: String,
    pub this_dc: i32,
    pub nearest_dc: i32,
}
