use std::time::Duration;

use tokio::time::Instant;

use super::Listen;
use crate::Client;
use crate::dispatcher::listeners::Listened;
use crate::errors::Result;
use crate::filters::Filter;
use crate::methods::SendMessage;
use crate::types::{Message, PeerRef, Text};

#[must_use = "a listener does nothing until it is awaited"]
/// Request built by [`Client::ask`].
pub struct Ask<'a> {
    prompt: SendMessage<'a>,
    listen: Listen<'a, Message>,
}

impl Client {
    /// Sends `text` to `chat_id`, then waits for the next message there, as `listen` does, and returns it.
    ///
    /// Only messages newer than the question count. Listening starts before the question goes out, so an answer
    /// that comes before `send_message` returns still counts. Other messages that match while the question is being
    /// sent reach the handlers only once it is out, and may then come after newer ones. To wait for a button press
    /// instead, send the keyboard with `send_message` and await `wait_for_callback_query(chat_id).message_id(sent.id)`.
    /// A question sent through a business connection gets no answer here: business messages never reach listeners.
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

    /// Also accepts an answer in this chat, besides the one the question went to; call again for more chats. Only
    /// messages newer than the question count in the question's own chat; any message counts in these.
    pub fn chat(mut self, chat_id: impl Into<PeerRef>) -> Self {
        self.listen = self.listen.chat(chat_id);
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

    /// How long to wait for the answer; the client's `listener_timeout`, 5 minutes unless set.
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
        let Ask { prompt, listen: Listen { client, mut spec } } = self;
        // Resolved before the question goes out, so a full budget or a client without updates leaves none
        // unanswered. The timeout starts once the question is out.
        spec.chats.push(prompt.chat_id().clone());
        let timeout = spec.timeout;
        let mut criteria = spec.resolve(client).await?;
        criteria.deadline = None;
        let listeners = &client.0.handlers.listeners;
        let id = listeners.next_id();
        // Keeps what arrives while the question is on its way. Dropping it (a failed send, or this future) removes
        // the listener and gives back what it kept.
        let hold = listeners.messages.hold(id, criteria)?;
        let sent = prompt.await?;
        let deadline = timeout.and_then(|timeout| Instant::now().checked_add(timeout));
        let me = client.0.me.lock().unwrap().clone();
        let answer = listeners.messages.release(id, (sent.chat.id, sent.id), deadline, &me).await;
        drop(hold);
        Ok(answer?.prepare().await)
    }
}

into_future!(Ask, Message);

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};
    use std::time::Duration;

    use crate::Client;
    use crate::dispatcher::listeners::Offer;
    use crate::dispatcher::listeners::tests::{message, until_released, until_waiting};
    use crate::errors::{Error, Result, RpcError};
    use crate::filters;
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
        until_released(&client.0.handlers.listeners.messages).await;
        (client, waiter)
    }

    /// Asks as `asking` does, but the question sleeps off a one-second flood wait before `reply` comes. On a paused
    /// clock that sleep lasts until the test awaits, so what it offers first arrives while the question is on its
    /// way.
    async fn asking_slowly(reply: Result<Vec<u8>>) -> (Client, tokio::task::JoinHandle<Result<Message>>) {
        let flood = Err(Error::Rpc(RpcError::parse(420, "FLOOD_WAIT_1")));
        let (client, _sent) = Client::scripted(vec![flood, reply]);
        client.0.me.lock().unwrap().id = 5;
        let waiter = tokio::spawn({
            let client = client.clone();
            async move { client.ask("me", "Your name?").await }
        });
        until_waiting(&client.0.handlers.listeners.messages).await;
        (client, waiter)
    }

    fn returned(client: &Client) -> Vec<i32> {
        client.0.handlers.listeners.messages.take_returned().iter().map(|m| m.id).collect()
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
        let markup = ForceReply::new().placeholder("name");
        let expected = markup.write().to_bytes();
        let waiter = tokio::spawn({
            let client = client.clone();
            async move { client.ask("me", "Your name?").prompt(|q| q.reply_markup(markup)).await }
        });
        until_released(&client.0.handlers.listeners.messages).await;
        let body = requests.try_recv().expect("the question was not sent");
        assert!(body.windows(expected.len()).any(|w| w == expected), "the reply markup is missing");
        waiter.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn an_answer_before_the_question_returns_is_kept() {
        let (client, waiter) = asking_slowly(Ok(sent(9))).await;
        assert!(offer(&client, 10, 5, true), "the answer came while the question was on its way");
        assert_eq!(waiter.await.unwrap().unwrap().id, 10);
    }

    #[tokio::test(start_paused = true)]
    async fn the_first_kept_message_newer_than_the_question_answers() {
        let (client, waiter) = asking_slowly(Ok(sent(9))).await;
        assert!(offer(&client, 8, 5, true));
        assert!(offer(&client, 10, 5, true));
        assert_eq!(waiter.await.unwrap().unwrap().id, 10);
        assert_eq!(returned(&client), [8], "the message older than the question goes on to the handlers");
        assert!(!client.0.handlers.listeners.messages.waiting());
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_send_leaves_no_listener() {
        let forbidden = Err(Error::Rpc(RpcError::parse(403, "CHAT_WRITE_FORBIDDEN")));
        let (client, waiter) = asking_slowly(forbidden).await;
        assert!(offer(&client, 10, 5, true));
        let got = waiter.await.unwrap();
        assert!(matches!(&got, Err(e) if e.is_chat_write_forbidden()), "{got:?}");
        assert!(!client.0.handlers.listeners.messages.waiting());
        assert_eq!(returned(&client), [10], "what it kept goes on to the handlers");
    }

    #[tokio::test(start_paused = true)]
    async fn dropping_ask_before_the_question_returns_leaves_no_listener() {
        let (client, waiter) = asking_slowly(Ok(sent(9))).await;
        assert!(offer(&client, 10, 5, true));
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        assert!(!client.0.handlers.listeners.messages.waiting());
        assert_eq!(returned(&client), [10]);
    }

    #[tokio::test(start_paused = true)]
    async fn a_message_whose_filter_ran_across_the_release_is_checked_again() {
        let (entered, go) = (Arc::new(Barrier::new(2)), Arc::new(Barrier::new(2)));
        let flood = Err(Error::Rpc(RpcError::parse(420, "FLOOD_WAIT_1")));
        let (client, _sent) = Client::scripted(vec![flood, Ok(sent(9))]);
        client.0.me.lock().unwrap().id = 5;
        // Message 8 matches the hold, then its filter waits until the question is out and the hold released.
        let filter = filters::create({
            let (entered, go) = (entered.clone(), go.clone());
            move |m: &Message| {
                if m.id == 8 {
                    entered.wait();
                    go.wait();
                }
                true
            }
        });
        let waiter = tokio::spawn({
            let client = client.clone();
            async move { client.ask("me", "Your name?").filter(filter).await }
        });
        let messages = &client.0.handlers.listeners.messages;
        until_waiting(messages).await;
        let late = std::thread::spawn({
            let client = client.clone();
            move || offer(&client, 8, 5, true)
        });
        entered.wait();
        // Idle, the paused clock jumps past the flood wait: the question gets id 9 and the hold ends.
        tokio::time::sleep(Duration::from_secs(2)).await;
        until_released(messages).await;
        go.wait();
        assert!(!late.join().unwrap(), "message 8 is older than the question");
        assert!(offer(&client, 10, 5, true));
        assert_eq!(waiter.await.unwrap().unwrap().id, 10);
    }

    #[tokio::test]
    async fn an_answer_in_an_extra_chat_counts() {
        let (client, _sent) = Client::scripted(vec![Ok(sent(9))]);
        client.0.me.lock().unwrap().id = 5;
        let waiter = tokio::spawn({
            let client = client.clone();
            async move { client.ask("me", "Your name?").chat(7).await }
        });
        until_released(&client.0.handlers.listeners.messages).await;
        assert!(!offer(&client, 10, 8, false), "a chat nobody asked about");
        assert!(offer(&client, 10, 7, false));
        assert_eq!(waiter.await.unwrap().unwrap().chat.id, 7);
    }

    #[tokio::test]
    async fn after_only_applies_to_the_question_chat() {
        let (client, _sent) = Client::scripted(vec![Ok(sent(9))]);
        client.0.me.lock().unwrap().id = 5;
        let waiter = tokio::spawn({
            let client = client.clone();
            async move { client.ask("me", "Your name?").chat(7).await }
        });
        until_released(&client.0.handlers.listeners.messages).await;
        assert!(!offer(&client, 3, 5, true), "older than the question, in its chat");
        assert!(offer(&client, 3, 7, false), "ids in another chat aren't compared with the question's");
        assert_eq!(waiter.await.unwrap().unwrap().chat.id, 7);
    }
}
