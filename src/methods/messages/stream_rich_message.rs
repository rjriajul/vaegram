use super::send_message_draft::DraftOptions;
use super::stream_message::{Kind, MessageStream};
use crate::Client;
use crate::errors::Result;
use crate::types::{Message, PeerRef};

/// Builder returned by [`Client::stream_rich_message`]. Nothing is sent until [`start`](StreamRichMessage::start).
#[must_use = "call .start() to begin streaming"]
pub struct StreamRichMessage<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    html: bool,
    options: DraftOptions,
}

/// A running stream of rich drafts. [`finish`](RichMessageStream::finish) sends the message.
///
/// Dropping the stream aborts it and sends no message. The last draft stays on screen until Telegram clears it, about
/// 30 seconds later; a draft the user stopped with [`keep_on_stop`](StreamRichMessage::keep_on_stop) set stays until
/// the bot sends its next message to the chat.
#[must_use = "dropping the stream aborts it"]
pub struct RichMessageStream(MessageStream);

impl Client {
    /// Streams a reply as live drafts of one rich message.
    ///
    /// Pushed chunks append to Markdown source, or HTML with [`html`](StreamRichMessage::html), and every draft shows
    /// all of it, as [`send_rich_message_draft`](Client::send_rich_message_draft) does. A draft Telegram refuses, for
    /// example while a `**` is still open, is skipped, and the next chunk sends the whole source again.
    /// [`finish`](RichMessageStream::finish) sends it as one message with
    /// [`send_rich_message`](Client::send_rich_message). There is no local length limit: a message too long for
    /// Telegram fails at `finish` with Telegram's error. Call [`start`](StreamRichMessage::start) to begin.
    pub fn stream_rich_message(&self, chat_id: impl Into<PeerRef>) -> StreamRichMessage<'_> {
        StreamRichMessage { client: self, chat_id: chat_id.into(), html: false, options: DraftOptions::default() }
    }
}

impl StreamRichMessage<'_> {
    /// Starts the background task that sends drafts, and returns the stream to push source text into.
    pub fn start(self) -> RichMessageStream {
        let kind = Kind::Rich { html: self.html };
        RichMessageStream(MessageStream::spawn(self.client, self.chat_id, self.options, kind))
    }

    /// Reads the pushed source as HTML.
    pub fn html(mut self) -> Self {
        self.html = true;
        self
    }

    /// Reads the pushed source as Markdown, the default.
    pub fn markdown(mut self) -> Self {
        self.html = false;
        self
    }

    /// Sends the drafts and the message into a forum topic.
    pub fn message_thread_id(mut self, message_thread_id: i32) -> Self {
        self.options.message_thread_id = Some(message_thread_id);
        self
    }

    /// Lets the user stop the draft. [`is_stopped`](RichMessageStream::is_stopped) reports the stop only when this
    /// is set.
    pub fn can_stop(mut self) -> Self {
        self.options.can_stop = true;
        self
    }

    /// Keeps the draft shown after the user stops it, until the bot sends its next message to the chat.
    pub fn keep_on_stop(mut self) -> Self {
        self.options.keep_on_stop = true;
        self
    }
}

impl RichMessageStream {
    /// Appends source text without waiting for the network.
    ///
    /// Returns `false` once the user pressed stop. A refused draft doesn't end the stream; only
    /// [`finish`](RichMessageStream::finish) can fail.
    pub fn push(&self, chunk: &str) -> bool {
        self.0.push(chunk)
    }

    /// Returns whether the user pressed stop on the draft. Only reported when `can_stop` is set.
    pub fn is_stopped(&self) -> bool {
        self.0.is_stopped()
    }

    /// Sends everything pushed so far as one rich message and returns it, or `None` when the source is empty.
    ///
    /// After a stop this is still the way to keep the partial text. Dropping the returned future (a timeout around
    /// it) doesn't stop the sending: the message still goes out, but nobody learns whether it did.
    pub async fn finish(self) -> Result<Option<Message>> {
        Ok(self.0.finish().await?.pop())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::sync::mpsc::UnboundedReceiver;

    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Identifiable, Serializable, base, functions, types};

    fn markdown(source: &str) -> Vec<u8> {
        let markdown =
            types::InputRichMessageMarkdown { rtl: false, noautolink: false, markdown: source.into(), files: None };
        base::InputRichMessage::InputRichMessageMarkdown(Box::new(markdown)).to_bytes()
    }

    fn html(source: &str) -> Vec<u8> {
        let html = types::InputRichMessageHTML { rtl: false, noautolink: false, html: source.into(), files: None };
        base::InputRichMessage::InputRichMessageHTML(Box::new(html)).to_bytes()
    }

    fn is_draft_of(body: &[u8], source: &str) -> bool {
        body[..4] == functions::messages::SetTyping::CONSTRUCTOR_ID.to_le_bytes() && body.ends_with(&markdown(source))
    }

    fn sent() -> Vec<u8> {
        let sent = types::UpdateShortSentMessage {
            out: true,
            id: 9,
            pts: 0,
            pts_count: 0,
            date: 0,
            media: None,
            entities: None,
            ttl_period: None,
        };
        base::Updates::UpdateShortSentMessage(Box::new(sent)).to_bytes()
    }

    // A stream that ended early never sends the request, so the wait has a deadline.
    async fn next(rx: &mut UnboundedReceiver<Vec<u8>>) -> Vec<u8> {
        let request = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await;
        request.expect("nothing sent").expect("the session closed")
    }

    #[tokio::test]
    async fn every_draft_carries_the_whole_source_then_one_rich_message_is_sent() {
        let accepted = || Ok(true.to_bytes());
        let (client, mut rx) = Client::scripted(vec![accepted(), accepted(), accepted(), Ok(sent())]);
        let stream = client.stream_rich_message("me").start();
        let mut source = String::new();
        for chunk in ["# Plan\n\n", "1. **Read** the ", "slice"] {
            assert!(stream.push(chunk));
            source.push_str(chunk);
            assert!(is_draft_of(&next(&mut rx).await, &source), "no draft of {source:?}");
        }
        assert_eq!(stream.finish().await.unwrap().map(|m| m.id), Some(9));
        let body = next(&mut rx).await;
        assert_eq!(body[..4], functions::messages::SendMessage::CONSTRUCTOR_ID.to_le_bytes());
        let flags = u32::from_le_bytes(body[4..8].try_into().unwrap());
        assert_ne!(flags & 1 << 23, 0);
        assert!(body.ends_with(&markdown(&source)));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_refused_draft_does_not_end_the_stream() {
        let refused = Err(Error::Rpc(RpcError::parse(400, "RICH_MESSAGE_INVALID")));
        let (client, mut rx) = Client::scripted(vec![refused, Ok(true.to_bytes()), Ok(sent())]);
        let stream = client.stream_rich_message("me").start();
        assert!(stream.push("Some **bo"));
        assert!(is_draft_of(&next(&mut rx).await, "Some **bo"));
        assert!(stream.push("ld** text"));
        assert!(is_draft_of(&next(&mut rx).await, "Some **bold** text"));
        assert_eq!(stream.finish().await.unwrap().map(|m| m.id), Some(9));
        assert!(next(&mut rx).await.ends_with(&markdown("Some **bold** text")));
    }

    #[tokio::test]
    async fn a_dropped_stream_sends_no_final_message() {
        let (client, mut rx) = Client::recording();
        let stream = client.stream_rich_message("me").start();
        assert!(stream.push("Some **bold** text"));
        assert!(is_draft_of(&next(&mut rx).await, "Some **bold** text"));
        drop(stream);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn html_streams_send_html_drafts_and_message() {
        let (client, mut rx) = Client::scripted(vec![Ok(true.to_bytes()), Ok(sent())]);
        let stream = client.stream_rich_message("me").html().start();
        assert!(stream.push("<b>hi</b>"));
        let draft = next(&mut rx).await;
        assert_eq!(draft[..4], functions::messages::SetTyping::CONSTRUCTOR_ID.to_le_bytes());
        assert!(draft.ends_with(&html("<b>hi</b>")));
        assert_eq!(stream.finish().await.unwrap().map(|m| m.id), Some(9));
        let body = next(&mut rx).await;
        assert_eq!(body[..4], functions::messages::SendMessage::CONSTRUCTOR_ID.to_le_bytes());
        assert!(body.ends_with(&html("<b>hi</b>")));
    }

    #[tokio::test]
    async fn a_long_source_is_sent_whole_as_one_message() {
        let (first, second) = ("a".repeat(3000), "b".repeat(3000));
        let whole = format!("{first}{second}");
        let (client, mut rx) = Client::scripted(vec![Ok(true.to_bytes()), Ok(true.to_bytes()), Ok(sent())]);
        let stream = client.stream_rich_message("me").start();
        assert!(stream.push(&first));
        assert!(is_draft_of(&next(&mut rx).await, &first));
        assert!(stream.push(&second));
        assert!(is_draft_of(&next(&mut rx).await, &whole));
        assert_eq!(stream.finish().await.unwrap().map(|m| m.id), Some(9));
        let body = next(&mut rx).await;
        assert_eq!(body[..4], functions::messages::SendMessage::CONSTRUCTOR_ID.to_le_bytes());
        assert!(body.ends_with(&markdown(&whole)));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn finishing_an_empty_stream_sends_nothing() {
        let (client, mut rx) = Client::recording();
        let stream = client.stream_rich_message("me").start();
        assert!(stream.finish().await.unwrap().is_none());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn finish_returns_telegrams_error_for_the_message() {
        let too_long = Err(Error::Rpc(RpcError::parse(400, "MESSAGE_TOO_LONG")));
        let (client, mut rx) = Client::scripted(vec![Ok(true.to_bytes()), too_long]);
        let stream = client.stream_rich_message("me").start();
        assert!(stream.push("Some **bold** text"));
        assert!(is_draft_of(&next(&mut rx).await, "Some **bold** text"));
        let refused = stream.finish().await;
        assert!(matches!(&refused, Err(Error::Rpc(e)) if e.name == "MESSAGE_TOO_LONG"), "{refused:?}");
        assert!(next(&mut rx).await.ends_with(&markdown("Some **bold** text")));
    }
}
