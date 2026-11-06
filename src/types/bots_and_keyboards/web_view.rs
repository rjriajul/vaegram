use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};

/// A mini app opened for the user.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct WebView {
    /// URL to load in the web view.
    pub url: String,
    /// Id of the web app query the bot answers. `None` when the view has no query to answer.
    pub query_id: Option<i64>,
    /// Set when the web view should open at full height.
    pub fullsize: bool,
    /// Set when the web view should open in fullscreen.
    pub fullscreen: bool,
    /// Telegram's `same_origin` flag on the result, passed through unchanged.
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
    /// The view to show. Keep it alive with [`prolong`](Self::prolong) while it stays open.
    pub view: WebView,
    request: functions::messages::ProlongWebView,
}

impl WebViewSession {
    pub(crate) fn new(view: WebView, request: functions::messages::ProlongWebView) -> WebViewSession {
        WebViewSession { view, request }
    }

    /// Keeps the web view alive.
    ///
    /// Call it every 60 seconds while the app is open. A `QUERY_ID_INVALID` error means it must be closed.
    pub async fn prolong(&self, client: &Client) -> Result<()> {
        client.invoke(&self.request).await.map(drop)
    }
}
