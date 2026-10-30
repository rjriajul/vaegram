use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebView {
    pub url: String,
    pub query_id: Option<i64>,
    pub fullsize: bool,
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

#[derive(Debug, Clone, PartialEq)]
pub struct WebViewSession {
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
