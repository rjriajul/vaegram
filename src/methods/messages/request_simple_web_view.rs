use super::web_view_options::{WebViewOptions, web_view_builder};
use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{PeerRef, WebView};

#[must_use = "a request does nothing until it is awaited"]
pub struct RequestSimpleWebView<'a> {
    client: &'a Client,
    bot_id: PeerRef,
    url: Option<String>,
    from_switch_webview: bool,
    from_side_menu: bool,
    options: WebViewOptions,
}

impl Client {
    pub fn request_simple_web_view(&self, bot_id: impl Into<PeerRef>) -> RequestSimpleWebView<'_> {
        RequestSimpleWebView {
            client: self,
            bot_id: bot_id.into(),
            url: None,
            from_switch_webview: false,
            from_side_menu: false,
            options: WebViewOptions::default(),
        }
    }
}

impl RequestSimpleWebView<'_> {
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn from_switch_webview(mut self) -> Self {
        self.from_switch_webview = true;
        self
    }

    pub fn from_side_menu(mut self) -> Self {
        self.from_side_menu = true;
        self
    }

    async fn send(self) -> Result<WebView> {
        let request = functions::messages::RequestSimpleWebView {
            from_switch_webview: self.from_switch_webview,
            from_side_menu: self.from_side_menu,
            compact: self.options.compact,
            fullscreen: self.options.fullscreen,
            bot: self.client.resolve_user(&self.bot_id).await?,
            url: self.url,
            start_param: self.options.start_param,
            theme_params: self.options.theme_params,
            platform: self.options.platform,
        };
        Ok(WebView::parse(self.client.invoke(&request).await?))
    }
}

web_view_builder!(RequestSimpleWebView);

into_future!(RequestSimpleWebView, WebView);
