use super::web_view_options::{WebViewOptions, web_view_builder};
use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{PeerRef, ReplyParameters, WebView, WebViewSession};

#[must_use = "a request does nothing until it is awaited"]
pub struct RequestWebView<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    bot_id: PeerRef,
    url: Option<String>,
    from_bot_menu: bool,
    disable_notification: bool,
    reply_to: Option<ReplyParameters>,
    send_as: Option<PeerRef>,
    options: WebViewOptions,
}

impl Client {
    /// While it stays open, call [`WebViewSession::prolong`] every 60 seconds.
    pub fn request_web_view(&self, chat_id: impl Into<PeerRef>, bot_id: impl Into<PeerRef>) -> RequestWebView<'_> {
        RequestWebView {
            client: self,
            chat_id: chat_id.into(),
            bot_id: bot_id.into(),
            url: None,
            from_bot_menu: false,
            disable_notification: false,
            reply_to: None,
            send_as: None,
            options: WebViewOptions::default(),
        }
    }
}

impl RequestWebView<'_> {
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn from_bot_menu(mut self) -> Self {
        self.from_bot_menu = true;
        self
    }

    pub fn disable_notification(mut self) -> Self {
        self.disable_notification = true;
        self
    }

    pub fn reply_to(mut self, reply: impl Into<ReplyParameters>) -> Self {
        self.reply_to = Some(reply.into());
        self
    }

    pub fn send_as(mut self, chat_id: impl Into<PeerRef>) -> Self {
        self.send_as = Some(chat_id.into());
        self
    }

    async fn send(self) -> Result<WebViewSession> {
        let client = self.client;
        let (chat_id, peer) = client.resolve(&self.chat_id).await?;
        let send_as = match self.send_as {
            Some(send_as) => Some(client.resolve_peer(send_as).await?),
            None => None,
        };
        let reply_to = match &self.reply_to {
            Some(reply) => Some(input_reply(client, reply, chat_id).await?),
            None => None,
        };
        let request = functions::messages::RequestWebView {
            from_bot_menu: self.from_bot_menu,
            silent: self.disable_notification,
            compact: self.options.compact,
            fullscreen: self.options.fullscreen,
            peer,
            bot: client.resolve_user(&self.bot_id).await?,
            url: self.url,
            start_param: self.options.start_param,
            theme_params: self.options.theme_params,
            platform: self.options.platform,
            reply_to,
            send_as,
        };
        let view = WebView::parse(client.invoke(&request).await?);
        // webViewResultUrl docs: inline button, menu button and attachment menu apps (all of this method's
        // modes) get the query_id, and the client must prolong it every 60 seconds.
        let query_id = view.query_id.ok_or(Error::Unexpected("messages.requestWebView sent no query_id"))?;
        let prolong = functions::messages::ProlongWebView {
            silent: request.silent,
            peer: request.peer,
            bot: request.bot,
            query_id,
            reply_to: request.reply_to,
            send_as: request.send_as,
        };
        Ok(WebViewSession::new(view, prolong))
    }
}

async fn input_reply(client: &Client, reply: &ReplyParameters, chat_id: i64) -> Result<base::InputReplyTo> {
    Ok(base::InputReplyTo::InputReplyToMessage(Box::new(reply.write(client, chat_id).await?)))
}

web_view_builder!(RequestWebView);

into_future!(RequestWebView, WebViewSession);

#[cfg(test)]
mod tests {
    use std::future::IntoFuture;

    use tokio::sync::mpsc::UnboundedReceiver;

    use super::input_reply;
    use crate::Client;
    use crate::errors::Result;
    use crate::raw::functions::messages;
    use crate::raw::{Serializable, base, types};
    use crate::types::ReplyParameters;

    const CHAT: i64 = -5;
    const BOT: i64 = 7;
    const THEME: &str = r##"{"bg_color":"#ffffff"}"##;

    fn bot_client(replies: Vec<Result<Vec<u8>>>) -> (Client, UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::scripted(replies);
        let bot = types::InputPeerUser { user_id: BOT, access_hash: 99 };
        client.0.peers.lock().unwrap().insert(BOT, base::InputPeer::InputPeerUser(Box::new(bot)));
        (client, rx)
    }

    fn chat() -> base::InputPeer {
        base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 5 })
    }

    fn input_bot() -> base::InputUser {
        base::InputUser::InputUser(Box::new(types::InputUser { user_id: BOT, access_hash: 99 }))
    }

    async fn sent<F: IntoFuture>(rx: &mut UnboundedReceiver<Vec<u8>>, call: F) -> Vec<u8> {
        let _ = call.await;
        rx.try_recv().expect("no request sent")
    }

    #[tokio::test]
    async fn request_web_view_writes_every_option() {
        let (client, mut rx) = bot_client(Vec::new());
        let call = client
            .request_web_view(CHAT, BOT)
            .url("https://example.org/app")
            .from_bot_menu()
            .disable_notification()
            .reply_to(42)
            .send_as(CHAT)
            .start_param("ref")
            .theme_params(THEME)
            .platform("desktop")
            .fullscreen();
        let reply = input_reply(&client, &ReplyParameters::new(42), CHAT).await.unwrap();
        let expected = messages::RequestWebView {
            from_bot_menu: true,
            silent: true,
            compact: false,
            fullscreen: true,
            peer: chat(),
            bot: input_bot(),
            url: Some("https://example.org/app".into()),
            start_param: Some("ref".into()),
            theme_params: Some(base::DataJSON::DataJSON(Box::new(types::DataJSON { data: THEME.into() }))),
            platform: "desktop".into(),
            reply_to: Some(reply),
            send_as: Some(chat()),
        };
        assert_eq!(sent(&mut rx, call).await, expected.to_bytes());
    }

    #[tokio::test]
    async fn request_app_web_view_uses_short_name_and_default_platform() {
        let (client, mut rx) = bot_client(Vec::new());
        let call = client.request_app_web_view(BOT, BOT, "game").write_allowed().compact();
        let app = types::InputBotAppShortName { bot_id: input_bot(), short_name: "game".into() };
        let expected = messages::RequestAppWebView {
            write_allowed: true,
            compact: true,
            fullscreen: false,
            peer: base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: BOT, access_hash: 99 })),
            app: base::InputBotApp::InputBotAppShortName(Box::new(app)),
            start_param: None,
            theme_params: None,
            platform: "unknown".into(),
        };
        assert_eq!(sent(&mut rx, call).await, expected.to_bytes());
    }

    #[tokio::test]
    async fn prolong_repeats_the_resolved_options() {
        let result = types::WebViewResultUrl {
            fullsize: false,
            fullscreen: false,
            same_origin: false,
            query_id: Some(3),
            url: "https://example.org/app".into(),
        };
        let answer = base::WebViewResult::WebViewResultUrl(Box::new(result)).to_bytes();
        let (client, mut rx) = bot_client(vec![Ok(answer)]);
        let call = client.request_web_view(CHAT, BOT).disable_notification().reply_to(42).send_as(CHAT);
        let session = call.await.unwrap();
        rx.try_recv().expect("no request sent");

        let _ = session.prolong(&client).await;
        let expected = messages::ProlongWebView {
            silent: true,
            peer: chat(),
            bot: input_bot(),
            query_id: 3,
            reply_to: Some(input_reply(&client, &ReplyParameters::new(42), CHAT).await.unwrap()),
            send_as: Some(chat()),
        };
        assert_eq!(rx.try_recv().expect("no prolong sent"), expected.to_bytes());
    }
}
