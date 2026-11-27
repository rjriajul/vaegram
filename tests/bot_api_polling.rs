//! The `getUpdates` poll of a client in Bot API mode against a local stub that keeps updates as Telegram does: an
//! update stays until a poll's offset passes it, so one confirmed too early is lost and one never confirmed comes
//! again.
#![cfg(feature = "bot-api")]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use vaegram::{Client, filters};

const TOKEN: &str = "123:ABC";
const USER: i64 = 7;

/// One `getUpdates` the stub got: its body and when.
type Poll = (Value, Instant);

/// What Telegram holds for the bot.
#[derive(Default)]
struct Telegram {
    /// Updates not yet confirmed, oldest first.
    updates: Vec<Value>,
    /// Answers the next polls get before any update, whole envelopes.
    errors: VecDeque<Value>,
}

type Shared = Arc<Mutex<Telegram>>;

struct Stub {
    url: String,
    telegram: Shared,
    polls: mpsc::UnboundedReceiver<Poll>,
}

impl Stub {
    async fn start(updates: Vec<Value>, errors: Vec<Value>) -> Stub {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (tx, polls) = mpsc::unbounded_channel();
        let telegram: Shared = Arc::new(Mutex::new(Telegram { updates, errors: errors.into() }));
        let shared = telegram.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(serve(stream, tx.clone(), shared.clone()));
            }
        });
        Stub { url, telegram, polls }
    }

    /// The next `getUpdates` asking from `offset`, skipping the others.
    async fn poll_from(&mut self, offset: i64) -> Poll {
        loop {
            let poll = tokio::time::timeout(Duration::from_secs(10), self.polls.recv())
                .await
                .expect("no such poll in time")
                .expect("the stub stopped");
            if poll.0["offset"] == json!(offset) {
                return poll;
            }
        }
    }

    /// The next `getUpdates`, whatever it asks.
    async fn next_poll(&mut self) -> Poll {
        tokio::time::timeout(Duration::from_secs(10), self.polls.recv())
            .await
            .expect("no poll in time")
            .expect("the stub stopped")
    }

    fn push(&self, update: Value) {
        self.telegram.lock().unwrap().updates.push(update);
    }
}

async fn serve(mut stream: TcpStream, polls: mpsc::UnboundedSender<Poll>, telegram: Shared) {
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    let head_end = loop {
        if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        match stream.read(&mut buf).await {
            Ok(0) | Err(_) => return,
            Ok(n) => data.extend_from_slice(&buf[..n]),
        }
    };
    let head = String::from_utf8_lossy(&data[..head_end]).into_owned();
    let length = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") { value.trim().parse::<usize>().ok() } else { None }
        })
        .unwrap_or(0);
    while data.len() < head_end + length {
        match stream.read(&mut buf).await {
            Ok(0) | Err(_) => return,
            Ok(n) => data.extend_from_slice(&buf[..n]),
        }
    }
    let path = head.split_whitespace().nth(1).unwrap_or_default().to_owned();
    let prefix = format!("/bot{TOKEN}/");
    assert!(path.starts_with(&prefix), "request to {path}");
    let method = &path[prefix.len()..];
    let body = &data[head_end..head_end + length];
    let body: Value = if length == 0 { json!({}) } else { serde_json::from_slice(body).unwrap() };
    let reply = match method {
        "getMe" => ok(json!({ "id": 99, "is_bot": true, "first_name": "Bot", "username": "stub_bot" })),
        "getUpdates" => {
            let _ = polls.send((body.clone(), Instant::now()));
            get_updates(&body, &telegram).await
        }
        _ => ok(json!(true)),
    }
    .to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

fn ok(result: Value) -> Value {
    json!({ "ok": true, "result": result })
}

/// `getUpdates` as Telegram answers it: the offset confirms every update below it, `-1` asks for the newest alone.
async fn get_updates(body: &Value, telegram: &Shared) -> Value {
    let offset = body["offset"].as_i64().unwrap_or(0);
    let found = {
        let mut telegram = telegram.lock().unwrap();
        if let Some(error) = telegram.errors.pop_front() {
            return error;
        }
        if offset == -1 {
            let last = telegram.updates.pop();
            telegram.updates.clear();
            return ok(Value::Array(last.into_iter().collect()));
        }
        telegram.updates.retain(|u| u["update_id"].as_i64().unwrap_or(0) >= offset);
        telegram.updates.clone()
    };
    if found.is_empty() {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    ok(Value::Array(found))
}

fn message(update_id: i64, text: &str) -> Value {
    let date = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    json!({
        "update_id": update_id,
        "message": {
            "message_id": update_id,
            "date": date,
            "chat": { "id": USER, "type": "private", "first_name": "Ann" },
            "from": { "id": USER, "is_bot": false, "first_name": "Ann" },
            "text": text
        }
    })
}

/// A client whose handler sends every message's text to the receiver it returns.
async fn client(url: &str, catch_up: bool) -> (Client, mpsc::UnboundedReceiver<String>) {
    let client = Client::builder("bot_api_polling", 1, "hash")
        .bot_api(TOKEN)
        .bot_api_url(url)
        .catch_up(catch_up)
        .start()
        .await
        .unwrap();
    let (tx, texts) = mpsc::unbounded_channel();
    client.on_message(filters::all(), move |m| {
        let tx = tx.clone();
        async move {
            let _ = tx.send(m.text().to_owned());
            Ok::<_, vaegram::Error>(())
        }
    });
    (client, texts)
}

async fn next_text(texts: &mut mpsc::UnboundedReceiver<String>) -> String {
    tokio::time::timeout(Duration::from_secs(10), texts.recv()).await.expect("no message in time").unwrap()
}

#[tokio::test]
async fn without_catch_up_the_backlog_is_confirmed_and_skipped() {
    let mut stub = Stub::start(vec![message(4, "older"), message(5, "old")], vec![]).await;
    let (client, mut texts) = client(&stub.url, false).await;
    let runner = client.clone();
    let run = tokio::spawn(async move { runner.run().await });

    let (first, _) = stub.next_poll().await;
    assert_eq!((&first["offset"], &first["limit"]), (&json!(-1), &json!(1)));
    stub.poll_from(6).await;
    stub.push(message(6, "new"));
    assert_eq!(next_text(&mut texts).await, "new");
    stub.poll_from(7).await;
    assert!(texts.try_recv().is_err(), "the backlog was handled");

    client.stop();
    tokio::time::timeout(Duration::from_secs(10), run).await.expect("run didn't stop").unwrap().unwrap();
}

#[tokio::test]
async fn updates_the_bridge_skips_are_confirmed_too() {
    let mut stub = Stub::start(vec![json!({ "update_id": 3 })], vec![]).await;
    let (client, _texts) = client(&stub.url, true).await;
    let runner = client.clone();
    let run = tokio::spawn(async move { runner.run().await });

    stub.poll_from(4).await;
    assert!(stub.telegram.lock().unwrap().updates.is_empty());

    client.stop();
    tokio::time::timeout(Duration::from_secs(10), run).await.expect("run didn't stop").unwrap().unwrap();
}

#[tokio::test]
async fn a_run_after_a_stop_goes_on_without_handling_an_update_twice() {
    let mut stub = Stub::start(vec![message(1, "one"), message(2, "two")], vec![]).await;
    let (client, mut texts) = client(&stub.url, true).await;
    let runner = client.clone();
    let run = tokio::spawn(async move { runner.run().await });
    let mut seen = vec![next_text(&mut texts).await];
    // Stopped with "two" queued or just taken, before a poll confirmed either to Telegram.
    client.stop();
    tokio::time::timeout(Duration::from_secs(10), run).await.expect("run didn't stop").unwrap().unwrap();

    while stub.polls.try_recv().is_ok() {}
    let runner = client.clone();
    let run = tokio::spawn(async move { runner.run().await });
    stub.poll_from(3).await;
    // A poll a little later, so an update polled again would have reached the handler by now.
    stub.poll_from(3).await;
    client.stop();
    tokio::time::timeout(Duration::from_secs(10), run).await.expect("run didn't stop").unwrap().unwrap();

    while let Ok(text) = texts.try_recv() {
        seen.push(text);
    }
    seen.sort();
    assert_eq!(seen, ["one", "two"]);
}

#[tokio::test]
async fn a_retry_after_delays_the_next_poll() {
    let flood = json!({
        "ok": false,
        "error_code": 429,
        "description": "Too Many Requests: retry after 1",
        "parameters": { "retry_after": 1 }
    });
    let mut stub = Stub::start(vec![], vec![flood]).await;
    let (client, _texts) = client(&stub.url, true).await;
    let runner = client.clone();
    let run = tokio::spawn(async move { runner.run().await });

    let (_, refused) = stub.next_poll().await;
    let (_, next) = stub.next_poll().await;
    assert!(next - refused >= Duration::from_millis(900), "polled again after {:?}", next - refused);

    client.stop();
    tokio::time::timeout(Duration::from_secs(10), run).await.expect("run didn't stop").unwrap().unwrap();
}

#[tokio::test]
async fn a_conflicting_poller_ends_run_with_the_error() {
    let conflict = json!({
        "ok": false,
        "error_code": 409,
        "description": "Conflict: terminated by other getUpdates request; make sure that only one bot instance is running"
    });
    let stub = Stub::start(vec![], vec![conflict]).await;
    let (client, _texts) = client(&stub.url, true).await;
    let ended = tokio::time::timeout(Duration::from_secs(10), client.run()).await.expect("run didn't end");
    let error = ended.unwrap_err();
    assert_eq!(error.rpc().map(|e| e.code), Some(409), "{error}");
}
