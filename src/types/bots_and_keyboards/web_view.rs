use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};

/// A mini app opened for the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebView {
    /// URL to load in the web view.
    pub url: String,
    /// Id for answering the web app query. `None` if Telegram sent none.
    pub query_id: Option<i64>,
    /// Opens the web view at full height.
    pub fullsize: bool,
    /// Opens the web view in fullscreen.
    pub fullscreen: bool,
    pub same_origin: bool,
}

impl WebView {
    pub(crate) fn parse(raw: base::WebViewResult) -> WebView {
        let base::WebViewResult::WebViewResultUrl(r) = raw;
        WebView {
            url: r.url,
            query_id: r.query_id,
            fullsize: r.fullsize,
            fullscreen: r.fullscreen,
            same_origin: r.same_origin,
        }
    }
}

/// A [`WebView`] together with the request that keeps it alive.
#[derive(Debug, Clone, PartialEq)]
pub struct WebViewSession {
    /// The web view.
    pub view: WebView,
    request: functions::messages::ProlongWebView,
}

impl WebViewSession {
    pub(crate) fn new(view: WebView, request: functions::messages::ProlongWebView) -> WebViewSession {
        WebViewSession { view, request }
    }

    /// Call it every 60 seconds while the app is open. A `QUERY_ID_INVALID` error means it must be closed.
    pub async fn prolong(&self, client: &Client) -> Result<()> {
        client.invoke(&self.request).await.map(drop)
    }
}
