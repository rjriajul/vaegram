use std::time::Duration;

use tokio::task::JoinHandle;

use crate::Client;
use crate::enums::ChatAction;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::PeerRef;

const REPEAT_DELAY: Duration = Duration::from_secs(4);

#[must_use = "a request does nothing until it is awaited"]
pub struct SendChatAction<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    action: ChatAction,
    business_connection_id: Option<String>,
    top_msg_id: Option<i32>,
}

/// Awaiting it sends the action once, then keeps sending it in the background until the [`ChatActionHandle`] is
/// stopped or dropped.
#[must_use = "a request does nothing until it is awaited"]
pub struct RepeatChatAction<'a> {
    send: SendChatAction<'a>,
    delay: Duration,
}

/// Keeps a chat action showing. The background task holds a clone of the client, which stays alive until the handle
/// is stopped or dropped. Stopping doesn't clear the indicator at once; send [`ChatAction::Cancel`] for that.
#[derive(Debug)]
#[must_use = "dropping the handle stops the action"]
pub struct ChatActionHandle {
    task: Option<JoinHandle<Result<()>>>,
}

impl Client {
    pub fn send_chat_action(&self, chat_id: impl Into<PeerRef>, action: ChatAction) -> SendChatAction<'_> {
        SendChatAction { client: self, chat_id: chat_id.into(), action, business_connection_id: None, top_msg_id: None }
    }
}

impl<'a> SendChatAction<'a> {
    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.business_connection_id = Some(id.into());
        self
    }

    /// Shows the action in this topic of a forum, or thread of a chat; a repeating action keeps it.
    pub fn message_thread_id(mut self, id: i32) -> Self {
        self.top_msg_id = Some(id);
        self
    }

    /// Repeats the action every 4 seconds, since Telegram shows it for about 5.
    pub fn repeat(self) -> RepeatChatAction<'a> {
        RepeatChatAction { send: self, delay: REPEAT_DELAY }
    }

    async fn send(self) -> Result<()> {
        self.request().await?;
        Ok(())
    }

    /// Sends the action once; returns the request and the business connection it went through.
    async fn request(self) -> Result<(functions::messages::SetTyping, Option<String>)> {
        let peer = self.client.resolve_peer(self.chat_id).await?;
        let request = functions::messages::SetTyping { peer, top_msg_id: self.top_msg_id, action: self.action.write() };
        let (_, request) = self.client.invoke_business(self.business_connection_id.as_deref(), request).await?;
        Ok((request, self.business_connection_id))
    }
}

impl RepeatChatAction<'_> {
    /// The time between two sends; it must not be zero.
    pub fn repeat_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    async fn send(self) -> Result<ChatActionHandle> {
        if self.delay.is_zero() {
            return Err(Error::InvalidArgument("the repeat delay must not be zero"));
        }
        // The first send is awaited, so a chat or right that fails comes back here rather than ending a task.
        let (client, delay) = (self.send.client.clone(), self.delay);
        let (mut request, connection) = self.send.request().await?;
        let task = tokio::spawn(async move {
            loop {
                tokio::time::sleep(delay).await;
                match client.invoke_business(connection.as_deref(), request).await {
                    Ok((_, sent)) => request = sent,
                    Err(e) => return Err::<(), _>(e),
                }
            }
        });
        Ok(ChatActionHandle { task: Some(task) })
    }
}

impl ChatActionHandle {
    /// Stops repeating. Returns the error that ended the repeats early, if one did.
    pub async fn stop(mut self) -> Result<()> {
        let Some(task) = self.task.take() else { return Ok(()) };
        task.abort();
        match task.await {
            Ok(ended) => ended,
            Err(e) if e.is_cancelled() => Ok(()),
            Err(e) => std::panic::resume_unwind(e.into_panic()),
        }
    }
}

impl Drop for ChatActionHandle {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

into_future!(SendChatAction, ());
into_future!(RepeatChatAction, ChatActionHandle);

impl crate::methods::OnBehalf for SendChatAction<'_> {
    fn business_connection(&mut self) -> &mut Option<String> {
        &mut self.business_connection_id
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::sync::mpsc::UnboundedReceiver;
    use tokio::time::sleep;

    use crate::Client;
    use crate::enums::ChatAction;
    use crate::errors::{Error, Result, RpcError};
    use crate::raw::{Serializable, base, functions, types};

    fn chat(replies: Vec<Result<Vec<u8>>>) -> (Client, UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::scripted(replies);
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        (client, rx)
    }

    fn sent(rx: &mut UnboundedReceiver<Vec<u8>>) -> Vec<Vec<u8>> {
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    fn forbidden() -> Result<Vec<u8>> {
        Err(Error::Rpc(RpcError { code: 403, name: "CHAT_WRITE_FORBIDDEN".into(), value: None }))
    }

    fn ok() -> Result<Vec<u8>> {
        Ok(true.to_bytes())
    }

    #[tokio::test]
    async fn a_business_chat_action_goes_through_the_connection() {
        let (client, mut rx) = Client::recording();
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        assert!(client.send_chat_action(7, ChatAction::Typing).business_connection_id("c").await.is_err());
        assert_eq!(rx.try_recv().expect("no request sent")[..4], 0xdd289f8e_u32.to_le_bytes());
    }

    #[tokio::test(start_paused = true)]
    async fn repeats_until_stopped() {
        let (client, mut rx) = chat(vec![ok(), ok(), ok()]);
        let handle = client.send_chat_action(7, ChatAction::Typing).repeat().await.unwrap();
        assert_eq!(sent(&mut rx).len(), 1);
        // Each check falls between two sends, so the order of timers due at the same instant doesn't matter.
        sleep(Duration::from_secs(6)).await;
        assert_eq!(sent(&mut rx).len(), 1);
        sleep(Duration::from_secs(4)).await;
        assert_eq!(sent(&mut rx).len(), 1);
        handle.stop().await.unwrap();
        sleep(Duration::from_secs(8)).await;
        assert!(sent(&mut rx).is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn dropping_the_handle_stops_it() {
        let (client, mut rx) = chat(vec![ok(), ok()]);
        drop(client.send_chat_action(7, ChatAction::Typing).repeat().await.unwrap());
        sleep(Duration::from_secs(20)).await;
        assert_eq!(sent(&mut rx).len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn the_first_failure_comes_back_at_once() {
        let (client, mut rx) = chat(vec![forbidden(), ok()]);
        let result = client.send_chat_action(7, ChatAction::Typing).repeat().await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "CHAT_WRITE_FORBIDDEN"), "{result:?}");
        sleep(Duration::from_secs(20)).await;
        assert_eq!(sent(&mut rx).len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_later_failure_ends_the_loop_and_stop_returns_it() {
        let (client, mut rx) = chat(vec![ok(), forbidden(), ok()]);
        let handle = client.send_chat_action(7, ChatAction::Typing).repeat().await.unwrap();
        sleep(Duration::from_secs(20)).await;
        assert_eq!(sent(&mut rx).len(), 2);
        let stopped = handle.stop().await;
        assert!(matches!(&stopped, Err(Error::Rpc(e)) if e.name == "CHAT_WRITE_FORBIDDEN"), "{stopped:?}");
    }

    #[tokio::test]
    async fn zero_delay_is_refused_without_a_request() {
        let (client, mut rx) = chat(vec![ok()]);
        let result = client.send_chat_action(7, ChatAction::Typing).repeat().repeat_delay(Duration::ZERO).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(sent(&mut rx).is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn a_business_repeat_goes_through_the_connection() {
        let (client, mut rx) = chat(vec![ok(), ok()]);
        let action = client.send_chat_action(7, ChatAction::Typing).business_connection_id("c");
        let handle = action.repeat().repeat_delay(Duration::from_secs(1)).await.unwrap();
        sleep(Duration::from_millis(1500)).await;
        handle.stop().await.unwrap();
        let sent = sent(&mut rx);
        assert_eq!(sent.len(), 2);
        assert!(sent.iter().all(|body| body[..4] == 0xdd289f8e_u32.to_le_bytes()));
        assert_eq!(sent[0], sent[1]);
    }

    #[tokio::test(start_paused = true)]
    async fn disconnect_ends_the_loop() {
        let (client, mut rx) = chat(vec![ok()]);
        let handle = client.send_chat_action(7, ChatAction::Typing).repeat().await.unwrap();
        client.disconnect();
        sleep(Duration::from_secs(20)).await;
        assert_eq!(sent(&mut rx).len(), 2, "one send after the close, which fails, then none");
        let stopped = handle.stop().await;
        assert!(matches!(stopped, Err(Error::Disconnected)), "{stopped:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn the_thread_reaches_set_typing() {
        let (client, mut rx) = chat(vec![ok(), ok(), ok()]);
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        let request = functions::messages::SetTyping {
            peer: base::InputPeer::InputPeerUser(Box::new(user)),
            top_msg_id: Some(42),
            action: ChatAction::Typing.write(),
        };
        let expected = request.to_bytes();
        let carries = |body: &Vec<u8>| body.windows(expected.len()).any(|w| w == expected);
        client.send_chat_action(7, ChatAction::Typing).message_thread_id(42).await.unwrap();
        assert!(sent(&mut rx).iter().all(carries), "the topic is missing");

        let handle = client.send_chat_action(7, ChatAction::Typing).message_thread_id(42).repeat().await.unwrap();
        sleep(Duration::from_secs(5)).await;
        handle.stop().await.unwrap();
        let bodies = sent(&mut rx);
        assert!(bodies.len() == 2 && bodies.iter().all(carries), "the repeat lost the topic");
    }
}
