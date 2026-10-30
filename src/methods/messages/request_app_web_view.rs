use super::web_view_options::{WebViewOptions, web_view_builder};
use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{PeerRef, WebView};

#[must_use = "a request does nothing until it is awaited"]
pub struct RequestAppWebView<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    bot_id: PeerRef,
    short_name: String,
    write_allowed: bool,
    options: WebViewOptions,
}

impl Client {
    pub fn request_app_web_view(
        &self,
        chat_id: impl Into<PeerRef>,
        bot_id: impl Into<PeerRef>,
        short_name: impl Into<String>,
    ) -> RequestAppWebView<'_> {
        RequestAppWebView {
            client: self,
            chat_id: chat_id.into(),
            bot_id: bot_id.into(),
            short_name: short_name.into(),
            write_allowed: false,
            options: WebViewOptions::default(),
        }
    }
}

impl RequestAppWebView<'_> {
    pub fn write_allowed(mut self) -> Self {
        self.write_allowed = true;
        self
    }

    async fn send(self) -> Result<WebView> {
        let client = self.client;
        let app = types::InputBotAppShortName {
            bot_id: client.resolve_user(&self.bot_id).await?,
            short_name: self.short_name,
        };
        let request = functions::messages::RequestAppWebView {
            write_allowed: self.write_allowed,
            compact: self.options.compact,
            fullscreen: self.options.fullscreen,
            peer: client.resolve_peer(self.chat_id).await?,
            app: base::InputBotApp::InputBotAppShortName(Box::new(app)),
            start_param: self.options.start_param,
            theme_params: self.options.theme_params,
            platform: self.options.platform,
        };
        Ok(WebView::parse(client.invoke(&request).await?))
    }
}

web_view_builder!(RequestAppWebView);

into_future!(RequestAppWebView, WebView);
