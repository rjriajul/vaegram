/// A mini app to open from a button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebAppInfo {
    pub url: String,
}

impl WebAppInfo {
    pub fn new(url: impl Into<String>) -> Self {
        WebAppInfo { url: url.into() }
    }
}
