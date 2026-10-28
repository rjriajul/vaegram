use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::Notify;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use super::send_message_draft::{DraftOptions, SendMessageDraft, draft_setters};
use crate::Client;
use crate::crypto::random;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::raw::base;
use crate::types::{Message, PeerRef};

// Drafts disappear after 30 seconds, so a stalled stream re-sends its preview before that.
const KEEPALIVE: Duration = Duration::from_secs(20);
const MAX_UTF16: usize = 4096;

#[must_use = "call .start() to begin streaming"]
pub struct StreamMessage<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    options: DraftOptions,
}

pub struct MessageStream {
    client: Client,
    shared: Arc<StreamShared>,
    task: Option<JoinHandle<Result<Vec<Message>>>>,
}

#[derive(Default)]
pub(crate) struct StreamShared {
    state: Mutex<State>,
    notify: Notify,
    stopped: AtomicBool,
}

#[derive(Default)]
struct State {
    done: Vec<String>,
    text: String,
    units: usize,
    draft_id: i64,
    dirty: bool,
    finishing: bool,
    plain: bool,
}

impl Client {
    /// Streams a reply as live drafts; long text continues in a new message every 4096 UTF-16 units.
    /// Parts are formatted separately, so with a parse mode keep each tag inside one chunk.
    pub fn stream_message(&self, chat_id: impl Into<PeerRef>) -> StreamMessage<'_> {
        StreamMessage { client: self, chat_id: chat_id.into(), options: DraftOptions::default() }
    }
}

impl StreamMessage<'_> {
    pub fn start(self) -> MessageStream {
        let state = State {
            draft_id: new_draft_id(),
            plain: self.options.parse_mode.unwrap_or(self.client.0.parse_mode) == ParseMode::Disabled,
            ..State::default()
        };
        let shared = Arc::new(StreamShared { state: Mutex::new(state), ..StreamShared::default() });
        let client = self.client.clone();
        let task = tokio::spawn(run(client.clone(), self.chat_id, self.options, shared.clone()));
        MessageStream { client, shared, task: Some(task) }
    }
}

draft_setters!(StreamMessage);

impl MessageStream {
    /// Appends a chunk without waiting for the network. Returns false once the user pressed stop or
    /// a send failed; `finish` then reports the failure.
    pub fn push(&self, chunk: &str) -> bool {
        if self.is_stopped() || self.task.as_ref().is_none_or(JoinHandle::is_finished) {
            return false;
        }
        self.shared.state.lock().unwrap().push(chunk);
        self.shared.notify.notify_one();
        true
    }

    pub fn is_stopped(&self) -> bool {
        self.shared.stopped.load(Ordering::Relaxed)
    }

    /// Sends everything pushed so far as regular messages and returns them. After a stop this is
    /// still the way to keep the partial text: a kept draft disappears once the bot sends a message.
    pub async fn finish(mut self) -> Result<Vec<Message>> {
        self.shared.state.lock().unwrap().finishing = true;
        self.shared.notify.notify_one();
        let Some(task) = self.task.take() else { return Ok(Vec::new()) };
        // The task is only ever aborted by Drop, so a join error here is a panic.
        task.await.unwrap_or_else(|e| std::panic::resume_unwind(e.into_panic()))
    }
}

impl Drop for MessageStream {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.client.0.streams.lock().unwrap().retain(|_, s| !Arc::ptr_eq(s, &self.shared));
    }
}

impl Client {
    /// Called for every update, so a stop press reaches its stream whether or not anyone handles it.
    pub(crate) fn stop_stream(&self, update: &base::Update) {
        let base::Update::UpdateUserTyping(u) = update else { return };
        let base::SendMessageAction::SendMessageStopDraftAction(action) = &u.action else { return };
        if let Some(stream) = self.0.streams.lock().unwrap().get(&action.random_id) {
            stream.stopped.store(true, Ordering::Relaxed);
            stream.notify.notify_one();
        }
    }
}

impl State {
    // Parts break between chunks so formatting stays intact. Only a plain chunk too long for a
    // message on its own is cut, between chars; with a parse mode it would fail at `finish`.
    fn push(&mut self, chunk: &str) {
        let mut units = chunk.encode_utf16().count();
        if self.units + units > MAX_UTF16 && !self.text.is_empty() {
            self.next_part();
        }
        let mut chunk = chunk;
        while self.plain && units > MAX_UTF16 {
            let (cut, cut_units) = utf16_prefix(chunk, MAX_UTF16);
            self.text.push_str(&chunk[..cut]);
            self.next_part();
            chunk = &chunk[cut..];
            units -= cut_units;
        }
        self.text.push_str(chunk);
        self.units += units;
        self.dirty = true;
    }

    fn next_part(&mut self) {
        self.done.push(std::mem::take(&mut self.text));
        self.units = 0;
        self.draft_id = new_draft_id();
    }

    fn take_draft(&mut self) -> Option<(i64, String)> {
        std::mem::take(&mut self.dirty).then(|| self.snapshot())
    }

    fn snapshot(&self) -> (i64, String) {
        (self.draft_id, self.text.clone())
    }
}

async fn run(
    client: Client,
    chat_id: PeerRef,
    options: DraftOptions,
    shared: Arc<StreamShared>,
) -> Result<Vec<Message>> {
    let mut sent = Vec::new();
    let mut resume = Instant::now();
    loop {
        let paused = Instant::now() < resume;
        let (done, tail, mut draft) = {
            let mut state = shared.state.lock().unwrap();
            let tail = state.finishing.then(|| std::mem::take(&mut state.text));
            let draft = if paused { None } else { state.take_draft() };
            (std::mem::take(&mut state.done), tail, draft)
        };
        let finishing = tail.is_some();
        for text in done.into_iter().chain(tail).filter(|t| !t.is_empty()) {
            sent.push(send(&client, &chat_id, options, text).await?);
        }
        if finishing {
            return Ok(sent);
        }
        if shared.stopped.load(Ordering::Relaxed) {
            shared.notify.notified().await;
            continue;
        }
        if paused {
            let _ = tokio::time::timeout_at(resume, shared.notify.notified()).await;
            continue;
        }
        if draft.is_none() && tokio::time::timeout(KEEPALIVE, shared.notify.notified()).await.is_err() {
            draft = Some(shared.state.lock().unwrap().snapshot()).filter(|(_, text)| !text.is_empty());
        }
        if let Some((draft_id, text)) = draft {
            if options.can_stop {
                client.0.streams.lock().unwrap().insert(draft_id, shared.clone());
            }
            // A lost preview is harmless; the final send reports real failures. invoke only sleeps
            // through short flood waits, so longer ones hold drafts back instead of failing per chunk.
            let text = text.into();
            let request = SendMessageDraft { client: &client, chat_id: chat_id.clone(), draft_id, text, options };
            if let Err(Error::Rpc(e)) = request.await {
                if e.code == 420 {
                    resume = Instant::now() + Duration::from_secs(e.value.unwrap_or(0).into());
                    shared.state.lock().unwrap().dirty = true;
                }
            }
        }
    }
}

async fn send(client: &Client, chat_id: &PeerRef, options: DraftOptions, text: String) -> Result<Message> {
    let mut message = client.send_message(chat_id.clone(), text);
    if let Some(mode) = options.parse_mode {
        message = message.parse_mode(mode);
    }
    if let Some(thread) = options.message_thread_id {
        message = message.message_thread_id(thread);
    }
    message.await
}

fn new_draft_id() -> i64 {
    // Bot API draft ids must be non-zero.
    i64::from_le_bytes(random()) | 1
}

fn utf16_prefix(s: &str, max: usize) -> (usize, usize) {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        if units + c.len_utf16() > max {
            return (i, units);
        }
        units += c.len_utf16();
    }
    (s.len(), units)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use super::{State, StreamShared};
    use crate::Client;
    use crate::raw::{base, types};

    #[test]
    fn sends_only_the_latest_snapshot() {
        let mut state = State::default();
        for chunk in ["Hel", "lo, ", "world"] {
            state.push(chunk);
        }
        assert_eq!(state.take_draft().map(|(_, text)| text).as_deref(), Some("Hello, world"));
        assert_eq!(state.take_draft(), None);
    }

    #[test]
    fn splits_between_chunks_by_utf16_units() {
        let mut state = State::default();
        state.push(&"a".repeat(4094));
        let first = state.draft_id;
        state.push("😀");
        assert!(state.done.is_empty());
        state.push("b");
        assert_eq!(state.done, ["a".repeat(4094) + "😀"]);
        assert_eq!((state.text.as_str(), state.units), ("b", 1));
        assert_ne!(state.draft_id, first);
    }

    #[test]
    fn cuts_an_oversized_plain_chunk_between_chars() {
        let mut state = State { plain: true, ..State::default() };
        state.push("x");
        state.push(&("a".repeat(4095) + "😀b"));
        assert_eq!(state.done, ["x".to_owned(), "a".repeat(4095)]);
        assert_eq!((state.text.as_str(), state.units), ("😀b", 3));
    }

    #[test]
    fn stop_press_reaches_its_stream_without_handlers() {
        let client = Client::offline();
        let stream = Arc::new(StreamShared::default());
        client.0.streams.lock().unwrap().insert(42, stream.clone());
        let stop = |random_id| {
            let action = types::SendMessageStopDraftAction { random_id };
            base::Update::UpdateUserTyping(Box::new(types::UpdateUserTyping {
                user_id: 7,
                top_msg_id: None,
                action: base::SendMessageAction::SendMessageStopDraftAction(Box::new(action)),
            }))
        };
        client.stop_stream(&stop(41));
        assert!(!stream.stopped.load(Ordering::Relaxed));
        client.stop_stream(&stop(42));
        assert!(stream.stopped.load(Ordering::Relaxed));
    }
}
