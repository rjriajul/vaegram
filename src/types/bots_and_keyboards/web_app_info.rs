/// A mini app to open from a button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebAppInfo {
    /// URL of the mini app.
    pub url: String,
}

impl WebAppInfo {
    /// Creates a mini app button target that opens `url`.
    pub fn new(url: impl Into<String>) -> Self {
        WebAppInfo { url: url.into() }
    }
}
