use std::time::Duration;

use super::Listen;
use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::methods::SendMessage;
use crate::types::{Message, PeerRef, Text};

#[must_use = "a listener does nothing until it is awaited"]
pub struct Ask<'a> {
    prompt: SendMessage<'a>,
    listen: Listen<'a, Message>,
}

impl Client {
    /// Sends `text` to `chat_id`, then waits for the next message there, as `listen` does, and returns it.
    ///
    /// Only messages newer than the question count. An answer processed before `send_message` returns is
    /// missed. To wait for a button press instead, send the keyboard with `send_message` and await
    /// `wait_for_callback_query(chat_id).message_id(sent.id)`.
    pub fn ask(&self, chat_id: impl Into<PeerRef>, text: impl Into<Text>) -> Ask<'_> {
        Ask { prompt: self.send_message(chat_id, text), listen: self.listen() }
    }
}

impl<'a> Ask<'a> {
    /// Sets any `send_message` option on the question, e.g. `.prompt(|q| q.reply_markup(keyboard))`.
    pub fn prompt(mut self, f: impl FnOnce(SendMessage<'a>) -> SendMessage<'a>) -> Self {
        self.prompt = f(self.prompt);
        self
    }

    /// Only answers from this user; call again to accept another user too.
    pub fn user(mut self, user_id: impl Into<PeerRef>) -> Self {
        self.listen = self.listen.user(user_id);
        self
    }

    /// Only answers this filter passes; replaces an earlier filter.
    pub fn filter(mut self, filter: Filter<Message>) -> Self {
        self.listen = self.listen.filter(filter);
        self
    }

    /// How long to wait for the answer; 5 minutes unless set.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.listen = self.listen.timeout(timeout);
        self
    }

    /// Waits for the answer for as long as it takes.
    pub fn no_timeout(mut self) -> Self {
        self.listen = self.listen.no_timeout();
        self
    }

    async fn send(self) -> Result<Message> {
        let Ask { prompt, mut listen } = self;
        let sent = prompt.await?;
        listen.spec.criteria.chats.push(sent.chat.id);
        listen.spec.criteria.after = Some(sent.id);
        listen.await
    }
}

into_future!(Ask, Message);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::dispatcher::listeners::Offer;
    use crate::dispatcher::listeners::tests::{message, until_waiting};
    use crate::errors::Result;
    use crate::raw::{Serializable, base, types};
    use crate::types::{ForceReply, Message};

    fn sent(id: i32) -> Vec<u8> {
        let short = types::UpdateShortSentMessage {
            out: true,
            id,
            pts: 0,
            pts_count: 0,
            date: 0,
            media: None,
            entities: None,
            ttl_period: None,
        };
        base::Updates::UpdateShortSentMessage(Box::new(short)).to_bytes()
    }

    /// Asks in Saved Messages (the account is 5), and the question gets id 9.
    async fn asking() -> (Client, tokio::task::JoinHandle<Result<Message>>) {
        let (client, _sent) = Client::scripted(vec![Ok(sent(9))]);
        client.0.me.lock().unwrap().id = 5;
        let waiter = tokio::spawn({
            let client = client.clone();
            async move { client.ask("me", "Your name?").await }
        });
        until_waiting(&client.0.handlers.listeners.messages).await;
        (client, waiter)
    }

    fn offer(client: &Client, id: i32, chat: i64, out: bool) -> bool {
        let me = client.0.me.lock().unwrap().clone();
        let offer = client.0.handlers.listeners.messages.offer(message(client, id, chat, None, out), &me);
        matches!(offer, Offer::Claimed)
    }

    #[tokio::test]
    async fn ask_returns_the_next_message_in_the_chat() {
        let (client, waiter) = asking().await;
        assert!(!offer(&client, 10, 7, false), "an answer from another chat");
        assert!(offer(&client, 10, 5, true));
        assert_eq!(waiter.await.unwrap().unwrap().id, 10);
    }

    #[tokio::test]
    async fn older_messages_do_not_answer() {
        let (client, waiter) = asking().await;
        assert!(!offer(&client, 8, 5, true));
        assert!(offer(&client, 11, 5, true));
        assert_eq!(waiter.await.unwrap().unwrap().id, 11);
    }

    #[tokio::test]
    async fn the_question_itself_is_not_the_answer() {
        let (client, waiter) = asking().await;
        assert!(!offer(&client, 9, 5, true));
        assert!(client.0.handlers.listeners.messages.waiting());
        waiter.abort();
    }

    #[tokio::test]
    async fn prompt_options_reach_the_request() {
        let (client, mut requests) = Client::scripted(vec![Ok(sent(9))]);
        let markup = ForceReply { selective: false, placeholder: Some("name".into()) };
        let expected = markup.write().to_bytes();
        let waiter = tokio::spawn({
            let client = client.clone();
            async move { client.ask("me", "Your name?").prompt(|q| q.reply_markup(markup)).await }
        });
        until_waiting(&client.0.handlers.listeners.messages).await;
        let body = requests.try_recv().expect("the question was not sent");
        assert!(body.windows(expected.len()).any(|w| w == expected), "the reply markup is missing");
        waiter.abort();
    }
}
