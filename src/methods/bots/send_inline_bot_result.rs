use crate::Client;
use crate::crypto::random;
use crate::errors::{Error, Result};
use crate::methods::messages::{SendOptions, send_builder};
use crate::raw::functions;
use crate::types::{Message, PeerRef};

/// Request built by [`Client::send_inline_bot_result`]. Awaiting it returns the sent [`Message`].
///
/// messages.sendInlineBotResult can't protect content, pay for broadcasts, play an effect or post a suggested post, so
/// awaiting fails with `Error::InvalidArgument` when [`protect_content`](SendInlineBotResult::protect_content),
/// [`allow_paid_broadcast`](SendInlineBotResult::allow_paid_broadcast),
/// [`message_effect_id`](SendInlineBotResult::message_effect_id) or
/// [`suggested_post_parameters`](SendInlineBotResult::suggested_post_parameters) is set.
#[must_use = "a request does nothing until it is awaited"]
pub struct SendInlineBotResult<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    query_id: i64,
    result_id: String,
    hide_via: bool,
    options: SendOptions,
}

impl Client {
    /// Sends a result picked from [`get_inline_bot_results`](Client::get_inline_bot_results) to a chat.
    ///
    /// `query_id` comes from those results and `result_id` from the picked entry.
    pub fn send_inline_bot_result(
        &self,
        chat_id: impl Into<PeerRef>,
        query_id: i64,
        result_id: impl Into<String>,
    ) -> SendInlineBotResult<'_> {
        SendInlineBotResult {
            client: self,
            chat_id: chat_id.into(),
            query_id,
            result_id: result_id.into(),
            hide_via: false,
            options: SendOptions::default(),
        }
    }
}

impl SendInlineBotResult<'_> {
    /// Hides the "via @bot" label on the sent message.
    pub fn hide_via(mut self) -> Self {
        self.hide_via = true;
        self
    }

    async fn send(self) -> Result<Message> {
        let options = &self.options;
        if options.protect_content
            || options.allow_paid_broadcast
            || options.message_effect_id.is_some()
            || options.suggested_post_parameters.is_some()
        {
            return Err(Error::InvalidArgument(
                "an inline bot result can't be protected, paid-broadcast, given an effect or suggested as a post",
            ));
        }
        let client = self.client;
        let (chat_id, peer) = client.resolve(&self.chat_id).await?;
        let reply_to = options.input_reply_to(client, chat_id, options.thread()).await?;
        let request = functions::messages::SendInlineBotResult {
            silent: options.disable_notification,
            background: false,
            clear_draft: false,
            hide_via: self.hide_via,
            peer,
            reply_to,
            // Telegram drops a second message with the same random_id, so retries can't double-send.
            random_id: i64::from_le_bytes(random()),
            query_id: self.query_id,
            id: self.result_id,
            schedule_date: options.schedule_date,
            send_as: None,
            quick_reply_shortcut: None,
            allow_paid_stars: options.paid_message_star_count,
        };
        let updates = client.invoke(&request).await?;
        Message::from_updates(client, updates, chat_id, (String::new(), None))
    }
}

send_builder!(SendInlineBotResult, Message);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Identifiable, base, functions, types};
    use crate::types::SuggestedPostParameters;

    fn client() -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::recording();
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        (client, rx)
    }

    #[tokio::test]
    async fn send_options_reach_the_request() {
        let (client, mut rx) = client();
        let send = client.send_inline_bot_result(7, 1, "r").reply_to(4).disable_notification().schedule_date(99);
        let _ = send.paid_message_star_count(5).await;
        let body = rx.try_recv().expect("no request sent");
        assert_eq!(
            u32::from_le_bytes(body[..4].try_into().unwrap()),
            functions::messages::SendInlineBotResult::CONSTRUCTOR_ID
        );
        // flags: reply_to (0), silent (5), schedule_date (10), allow_paid_stars (21).
        let flags = u32::from_le_bytes(body[4..8].try_into().unwrap());
        assert_eq!(flags, 1 | 1 << 5 | 1 << 10 | 1 << 21);
    }

    #[tokio::test]
    async fn options_the_request_cannot_carry_are_refused() {
        let (client, mut rx) = client();
        let refused = [
            client.send_inline_bot_result(7, 1, "r").protect_content().await,
            client.send_inline_bot_result(7, 1, "r").allow_paid_broadcast().await,
            client.send_inline_bot_result(7, 1, "r").message_effect_id(1).await,
            client.send_inline_bot_result(7, 1, "r").suggested_post_parameters(SuggestedPostParameters::new()).await,
        ];
        for (i, result) in refused.into_iter().enumerate() {
            assert!(matches!(result, Err(Error::InvalidArgument(_))), "{i}");
        }
        assert!(rx.try_recv().is_err());
    }
}
