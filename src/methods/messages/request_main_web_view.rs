use super::web_view_options::{WebViewOptions, web_view_builder};
use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{PeerRef, WebView};

#[must_use = "a request does nothing until it is awaited"]
pub struct RequestMainWebView<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    bot_id: PeerRef,
    options: WebViewOptions,
}

impl Client {
    pub fn request_main_web_view(
        &self,
        chat_id: impl Into<PeerRef>,
        bot_id: impl Into<PeerRef>,
    ) -> RequestMainWebView<'_> {
        RequestMainWebView {
            client: self,
            chat_id: chat_id.into(),
            bot_id: bot_id.into(),
            options: WebViewOptions::default(),
        }
    }
}

impl RequestMainWebView<'_> {
    async fn send(self) -> Result<WebView> {
        let client = self.client;
        let request = functions::messages::RequestMainWebView {
            compact: self.options.compact,
            fullscreen: self.options.fullscreen,
            peer: client.resolve_peer(self.chat_id).await?,
            bot: client.resolve_user(&self.bot_id).await?,
            start_param: self.options.start_param,
            theme_params: self.options.theme_params,
            platform: self.options.platform,
        };
        Ok(WebView::parse(client.invoke(&request).await?))
    }
}

web_view_builder!(RequestMainWebView);

into_future!(RequestMainWebView, WebView);
