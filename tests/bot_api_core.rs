//! A client in Bot API mode against a local HTTP stub: which Bot API method each friendly method sends and what
//! it returns, and a handler replying to a message `getUpdates` brought.
#![cfg(feature = "bot-api")]

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use vaegram::enums::ChatAction;
use vaegram::types::BotCommand;
use vaegram::{Client, filters};

const TOKEN: &str = "123:ABC";
const BOT: i64 = 99;
const USER: i64 = 7;

/// One request the stub got: the Bot API method and its JSON body.
type Call = (String, Value);

/// What the first `getUpdates` answers; later ones wait a little and answer nothing.
type Pending = Arc<Mutex<Option<Vec<Value>>>>;

struct Stub {
    url: String,
    calls: mpsc::UnboundedReceiver<Call>,
}

impl Stub {
    async fn start(updates: Vec<Value>) -> Stub {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (tx, calls) = mpsc::unbounded_channel();
        let pending: Pending = Arc::new(Mutex::new(Some(updates)));
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(serve(stream, tx.clone(), pending.clone()));
            }
        });
        Stub { url, calls }
    }

    /// The next request that `matches`, skipping the others.
    async fn next_where(&mut self, matches: impl Fn(&Call) -> bool) -> Call {
        loop {
            let call = tokio::time::timeout(Duration::from_secs(10), self.calls.recv())
                .await
                .expect("no such request in time")
                .expect("the stub stopped");
            if matches(&call) {
                return call;
            }
        }
    }

    /// The next request a friendly method made, past the ones the client makes on its own: `getMe` at start,
    /// `getChat` to look a peer up, and `getUpdates`.
    async fn next(&mut self) -> Call {
        self.next_where(|(method, _)| !matches!(method.as_str(), "getMe" | "getChat" | "getUpdates")).await
    }
}

async fn serve(mut stream: TcpStream, calls: mpsc::UnboundedSender<Call>, pending: Pending) {
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
    let method = path[prefix.len()..].to_owned();
    let body = if length == 0 { json!({}) } else { serde_json::from_slice(&data[head_end..head_end + length]).unwrap() };
    let _ = calls.send((method.clone(), body.clone()));
    let result = answer(&method, &body, &pending).await;
    let reply = json!({ "ok": true, "result": result }).to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

async fn answer(method: &str, body: &Value, pending: &Pending) -> Value {
    match method {
        "getMe" => json!({ "id": BOT, "is_bot": true, "first_name": "Bot", "username": "stub_bot" }),
        "getChat" => chat(body["chat_id"].as_i64().unwrap_or(0)),
        "getUpdates" => {
            let first = pending.lock().unwrap().take();
            match first {
                Some(updates) => Value::Array(updates),
                None => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    json!([])
                }
            }
        }
        "sendMessage" => message(50, body),
        "editMessageText" => message(body["message_id"].as_i64().unwrap_or(0), body),
        _ => json!(true),
    }
}

fn chat(id: i64) -> Value {
    if id > 0 {
        json!({ "id": id, "type": "private", "first_name": "Ann" })
    } else {
        json!({ "id": id, "type": "supergroup", "title": "Group" })
    }
}

/// The bot's message as the Bot API returns it after a send or an edit.
fn message(id: i64, body: &Value) -> Value {
    json!({
        "message_id": id,
        "date": unix_now(),
        "chat": chat(body["chat_id"].as_i64().unwrap_or(0)),
        "from": { "id": BOT, "is_bot": true, "first_name": "Bot" },
        "text": body["text"],
    })
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

async fn client(url: &str) -> Client {
    Client::builder("bot_api_core", 1, "hash").bot_api(TOKEN).bot_api_url(url).catch_up(true).start().await.unwrap()
}

#[tokio::test]
async fn friendly_methods_go_out_as_bot_api_methods() {
    let mut stub = Stub::start(Vec::new()).await;
    let client = client(&stub.url).await;

    let me = client.get_me().await.unwrap();
    assert_eq!((me.id, me.is_bot, me.username.as_deref()), (BOT, true, Some("stub_bot")));

    let sent = client.send_message(USER, "hello").await.unwrap();
    assert_eq!((sent.id, sent.chat.id, sent.text()), (50, USER, "hello"));
    let (method, body) = stub.next().await;
    assert_eq!(method, "sendMessage");
    assert_eq!((&body["chat_id"], &body["text"]), (&json!(USER), &json!("hello")));

    let edited = client.edit_message_text(USER, 50, "edited").await.unwrap();
    assert_eq!((edited.id, edited.text()), (50, "edited"));
    let (method, body) = stub.next().await;
    assert_eq!(method, "editMessageText");
    assert_eq!((&body["chat_id"], &body["message_id"], &body["text"]), (&json!(USER), &json!(50), &json!("edited")));

    client.send_chat_action(USER, ChatAction::Typing).await.unwrap();
    let (method, body) = stub.next().await;
    assert_eq!(method, "sendChatAction");
    assert_eq!((&body["chat_id"], &body["action"]), (&json!(USER), &json!("typing")));

    client.pin_chat_message(USER, 50).await.unwrap();
    let (method, body) = stub.next().await;
    assert_eq!(method, "pinChatMessage");
    assert_eq!((&body["chat_id"], &body["message_id"]), (&json!(USER), &json!(50)));

    client.answer_callback_query(5).text("done").await.unwrap();
    let (method, body) = stub.next().await;
    assert_eq!(method, "answerCallbackQuery");
    assert_eq!((&body["callback_query_id"], &body["text"]), (&json!("5"), &json!("done")));

    client.set_bot_commands([BotCommand::new("start", "Start the bot")]).await.unwrap();
    let (method, body) = stub.next().await;
    assert_eq!(method, "setMyCommands");
    assert_eq!(body["commands"], json!([{ "command": "start", "description": "Start the bot" }]));
    assert_eq!(body["scope"], json!({ "type": "default" }));

    // A private chat's message names no chat in TL: the bridge knows it from the send above.
    client.delete_messages(USER, &[50]).await.unwrap();
    let (method, body) = stub.next().await;
    assert_eq!(method, "deleteMessages");
    assert_eq!((&body["chat_id"], &body["message_ids"]), (&json!(USER), &json!([50])));
}

#[tokio::test]
async fn a_handler_replies_to_a_message_from_get_updates() {
    let update = json!({
        "update_id": 1,
        "message": {
            "message_id": 10,
            "date": unix_now(),
            "chat": { "id": USER, "type": "private", "first_name": "Ann" },
            "from": { "id": USER, "is_bot": false, "first_name": "Ann" },
            "text": "hi bot"
        }
    });
    let mut stub = Stub::start(vec![update]).await;
    let client = client(&stub.url).await;
    client.on_message(filters::all(), |m| async move { m.reply("hello back").await });
    let runner = client.clone();
    let run = tokio::spawn(async move { runner.run().await });

    let (method, body) = stub.next().await;
    assert_eq!(method, "sendMessage");
    assert_eq!((&body["chat_id"], &body["text"]), (&json!(USER), &json!("hello back")));
    // The next poll confirms the update only once the dispatcher took it.
    let (_, poll) = stub.next_where(|(method, body)| method == "getUpdates" && body["offset"] == json!(2)).await;
    assert_eq!(poll["allowed_updates"][0], json!("message"));

    client.stop();
    tokio::time::timeout(Duration::from_secs(10), run).await.expect("run didn't stop").unwrap().unwrap();
}
