//! Webhook mode of a client in Bot API mode, against a local Bot API stub: the webhook methods' requests, the
//! receiver's secret check, updates reaching the same handlers as polling does, repeats dropped, and no `getUpdates`
//! while a webhook feeds the run.
#![cfg(feature = "bot-api")]

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Notify, mpsc};
use vaegram::bot_api::{Webhook, WebhookResponse};
use vaegram::{Client, Error, filters};

const TOKEN: &str = "123:ABC";
const USER: i64 = 7;
const SECRET: &str = "s3cret_token-OK";
const HEADER: &str = "X-Telegram-Bot-Api-Secret-Token";

/// One request the stub got: the Bot API method and its JSON body.
type Call = (String, Value);

struct Stub {
    url: String,
    calls: mpsc::UnboundedReceiver<Call>,
}

impl Stub {
    async fn start() -> Stub {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (tx, calls) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(serve(stream, tx.clone()));
            }
        });
        Stub { url, calls }
    }

    /// The next request for `method`, skipping the others.
    async fn next(&mut self, method: &str) -> Value {
        loop {
            let (name, body) = tokio::time::timeout(Duration::from_secs(10), self.calls.recv())
                .await
                .expect("no such request in time")
                .expect("the stub stopped");
            if name == method {
                return body;
            }
        }
    }

    /// Whether a request for `method` came, among those already in.
    fn got(&mut self, method: &str) -> bool {
        let mut got = false;
        while let Ok((name, _)) = self.calls.try_recv() {
            got |= name == method;
        }
        got
    }
}

async fn serve(mut stream: TcpStream, calls: mpsc::UnboundedSender<Call>) {
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
    let body = &data[head_end..head_end + length];
    let body: Value = if length == 0 { json!({}) } else { serde_json::from_slice(body).unwrap_or(Value::Null) };
    let result = match method.as_str() {
        "getMe" => json!({ "id": 99, "is_bot": true, "first_name": "Bot", "username": "stub_bot" }),
        "getUpdates" => {
            tokio::time::sleep(Duration::from_millis(50)).await;
            json!([])
        }
        "getWebhookInfo" => json!({
            "url": "https://bot.example.com/telegram",
            "has_custom_certificate": false,
            "pending_update_count": 3,
            "last_error_date": 1_700_000_000,
            "last_error_message": "Wrong response from the webhook: 503 Service Unavailable",
            "max_connections": 40,
            "allowed_updates": ["message"]
        }),
        _ => json!(true),
    };
    let _ = calls.send((method, body));
    let reply = json!({ "ok": true, "result": result }).to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

fn message(update_id: i64, text: &str) -> Vec<u8> {
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
    .to_string()
    .into_bytes()
}

/// A client whose handler sends every message's text to the receiver it returns.
async fn client(url: &str) -> (Client, mpsc::UnboundedReceiver<String>) {
    let client = Client::builder("bot_api_webhook", 1, "hash").bot_api(TOKEN).bot_api_url(url).start().await.unwrap();
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

/// Delivers `body` with the right secret as Telegram does: again while the receiver answers 503.
async fn deliver(webhook: &Webhook, body: &[u8]) -> WebhookResponse {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match webhook.handle(body, [(HEADER, SECRET)]).await {
                WebhookResponse::Unavailable => tokio::time::sleep(Duration::from_millis(20)).await,
                answer => return answer,
            }
        }
    })
    .await
    .expect("the receiver never took the update")
}

/// Holds `run` in its start hook, before it takes any update, until the returned gate is opened; the receiver gets a
/// message as the hook starts.
fn hold_start(client: &Client) -> (Arc<Notify>, mpsc::UnboundedReceiver<()>) {
    let gate = Arc::new(Notify::new());
    let (started_tx, started) = mpsc::unbounded_channel();
    let held = gate.clone();
    client.on_start(move |_| {
        let (held, started_tx) = (held.clone(), started_tx.clone());
        async move {
            let _ = started_tx.send(());
            held.notified().await;
            Ok::<_, vaegram::Error>(())
        }
    });
    (gate, started)
}

#[tokio::test]
async fn a_repeat_of_an_update_still_waiting_for_room_is_not_called_taken() {
    let stub = Stub::start().await;
    let builder = Client::builder("bot_api_webhook", 1, "hash").bot_api(TOKEN).bot_api_url(&stub.url);
    let client = builder.updates_capacity(1).start().await.unwrap();
    let (tx, mut texts) = mpsc::unbounded_channel();
    client.on_message(filters::all(), move |m| {
        let tx = tx.clone();
        async move {
            let _ = tx.send(m.text().to_owned());
            Ok::<_, vaegram::Error>(())
        }
    });
    // The run takes nothing until the gate opens, so its queue, with room for one update, fills up.
    let (gate, mut started) = hold_start(&client);
    let webhook = client.webhook(SECRET).unwrap();
    let runner = webhook.clone();
    let run = tokio::spawn(async move { runner.run().await });
    started.recv().await.unwrap();

    assert_eq!(deliver(&webhook, &message(1, "one")).await, WebhookResponse::Accepted);
    let first = webhook.clone();
    let waiting = tokio::spawn(async move { first.handle(&message(2, "two"), [(HEADER, SECRET)]).await });
    // Long enough for that delivery to be waiting for room.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!waiting.is_finished(), "the queue had room");
    // Telegram trying again meanwhile: a 200 here would end its retries even if the first delivery failed.
    let again = webhook.handle(&message(2, "two"), [(HEADER, SECRET)]).await;
    assert_eq!(again, WebhookResponse::Unavailable);

    gate.notify_one();
    let first = tokio::time::timeout(Duration::from_secs(10), waiting).await.expect("still waiting").unwrap();
    assert_eq!(first, WebhookResponse::Accepted);
    let mut texts_seen = vec![next_text(&mut texts).await, next_text(&mut texts).await];
    texts_seen.sort();
    assert_eq!(texts_seen, ["one", "two"]);
    // Queued now, so a later repeat is a duplicate and runs nothing.
    assert_eq!(webhook.handle(&message(2, "two"), [(HEADER, SECRET)]).await, WebhookResponse::Duplicate);
    assert!(tokio::time::timeout(Duration::from_millis(300), texts.recv()).await.is_err(), "handled twice");

    client.stop();
    tokio::time::timeout(Duration::from_secs(10), run).await.expect("run didn't stop").unwrap().unwrap();
}

#[tokio::test]
async fn a_refused_webhook_run_leaves_a_polling_run_polling() {
    let mut stub = Stub::start().await;
    let (client, _texts) = client(&stub.url).await;
    let (gate, mut started) = hold_start(&client);
    let runner = client.clone();
    let run = tokio::spawn(async move { runner.run().await });
    // The polling run owns the client and hasn't decided yet whether to poll.
    started.recv().await.unwrap();

    let webhook = client.webhook(SECRET).unwrap();
    let refused = webhook.run().await;
    assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{refused:?}");

    gate.notify_one();
    stub.next("getUpdates").await;
    client.stop();
    tokio::time::timeout(Duration::from_secs(10), run).await.expect("run didn't stop").unwrap().unwrap();
}

#[tokio::test]
async fn a_missing_or_wrong_secret_is_refused_before_any_handler() {
    let stub = Stub::start().await;
    let (client, mut texts) = client(&stub.url).await;
    let webhook = client.webhook(SECRET).unwrap();
    let runner = webhook.clone();
    let run = tokio::spawn(async move { runner.run().await });

    let body = message(1, "forged");
    let none = webhook.handle(&body, std::iter::empty::<(&str, &str)>()).await;
    assert_eq!((none, none.status()), (WebhookResponse::Unauthorized, 401));
    let wrong = webhook.handle(&body, [(HEADER, "s3cret_token-OX")]).await;
    assert_eq!(wrong, WebhookResponse::Unauthorized);
    let longer = webhook.handle(&body, [(HEADER, "s3cret_token-OKK")]).await;
    assert_eq!(longer, WebhookResponse::Unauthorized);
    // The header's name compares without case, as HTTP header names do.
    assert_eq!(deliver(&webhook, &message(2, "real")).await, WebhookResponse::Accepted);
    assert_eq!(next_text(&mut texts).await, "real");
    assert!(texts.try_recv().is_err(), "a forged update reached the handler");

    client.stop();
    tokio::time::timeout(Duration::from_secs(10), run).await.expect("run didn't stop").unwrap().unwrap();
}

#[tokio::test]
async fn an_update_reaches_the_handler_once_and_nothing_is_polled() {
    let mut stub = Stub::start().await;
    let (client, mut texts) = client(&stub.url).await;
    let webhook = client.webhook(SECRET).unwrap();
    // No run takes updates yet: Telegram is told to come back later.
    let early = webhook.handle(&message(1, "early"), [("x-telegram-bot-api-secret-token", SECRET)]).await;
    assert_eq!((early, early.status()), (WebhookResponse::Unavailable, 503));

    let runner = webhook.clone();
    let run = tokio::spawn(async move { runner.run().await });
    assert_eq!(deliver(&webhook, &message(1, "early")).await, WebhookResponse::Accepted);
    assert_eq!(next_text(&mut texts).await, "early");
    // Telegram delivering it again, after a timeout on its side.
    let again = deliver(&webhook, &message(1, "early")).await;
    assert_eq!((again, again.status()), (WebhookResponse::Duplicate, 200));
    assert_eq!(deliver(&webhook, &message(2, "next")).await, WebhookResponse::Accepted);
    assert_eq!(next_text(&mut texts).await, "next");
    assert!(texts.try_recv().is_err(), "a repeated update ran its handler twice");
    // Something Telegram sent that isn't an update is taken and dropped, not retried forever.
    let junk = webhook.handle(b"not json", [(HEADER, SECRET)]).await;
    assert_eq!((junk, junk.status()), (WebhookResponse::Unreadable, 200));
    assert!(!stub.got("getUpdates"), "a run fed by a webhook polled");

    client.stop();
    tokio::time::timeout(Duration::from_secs(10), run).await.expect("run didn't stop").unwrap().unwrap();

    // Once the webhook's run is over, `Client::run` polls again.
    let runner = client.clone();
    let run = tokio::spawn(async move { runner.run().await });
    stub.next("getUpdates").await;
    client.stop();
    tokio::time::timeout(Duration::from_secs(10), run).await.expect("run didn't stop").unwrap().unwrap();
}

#[tokio::test]
async fn webhook_methods_send_what_telegram_expects() {
    let mut stub = Stub::start().await;
    let (client, _texts) = client(&stub.url).await;

    client.set_webhook("https://bot.example.com/telegram").max_connections(10).drop_pending_updates().await.unwrap();
    let body = stub.next("setWebhook").await;
    assert_eq!(body["url"], "https://bot.example.com/telegram");
    assert_eq!(body["max_connections"], 10);
    assert_eq!(body["drop_pending_updates"], true);
    // Sent every time, so a list set earlier never lingers.
    let kinds = [
        "message",
        "edited_message",
        "channel_post",
        "edited_channel_post",
        "callback_query",
        "inline_query",
        "business_connection",
        "business_message",
        "edited_business_message",
    ];
    assert_eq!(body["allowed_updates"], json!(kinds));
    assert!(body.get("secret_token").is_none());

    let webhook = client.webhook(SECRET).unwrap();
    webhook.register("https://bot.example.com/hook").allowed_updates(["message"]).await.unwrap();
    let body = stub.next("setWebhook").await;
    assert_eq!(body["secret_token"], SECRET);
    assert_eq!(body["allowed_updates"], json!(["message"]));

    let refused = client.set_webhook("https://bot.example.com/telegram").secret_token("has space").await;
    assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{refused:?}");
    assert!(!stub.got("setWebhook"), "a refused secret token was sent");
    assert!(matches!(client.webhook(""), Err(Error::InvalidArgument(_))));
    assert!(matches!(client.webhook("x".repeat(257)), Err(Error::InvalidArgument(_))));

    client.delete_webhook().drop_pending_updates().await.unwrap();
    assert_eq!(stub.next("deleteWebhook").await, json!({ "drop_pending_updates": true }));
    client.delete_webhook().await.unwrap();
    assert_eq!(stub.next("deleteWebhook").await, json!({}));

    let info = client.get_webhook_info().await.unwrap();
    assert_eq!(info.url, "https://bot.example.com/telegram");
    assert_eq!(info.pending_update_count, 3);
    assert_eq!(info.last_error_date, Some(1_700_000_000));
    assert_eq!(info.last_error_message.as_deref(), Some("Wrong response from the webhook: 503 Service Unavailable"));
    assert_eq!(info.max_connections, Some(40));
    assert_eq!(info.allowed_updates, ["message"]);
    assert!(info.ip_address.is_none());
}

/// The status line's code of what the server at `addr` answers a request to; reads until the server closes.
#[cfg(feature = "webhook-server")]
async fn status_of(addr: std::net::SocketAddr, method: &str, secret: Option<&str>, body: &[u8]) -> u16 {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let secret = secret.map(|s| format!("{HEADER}: {s}\r\n")).unwrap_or_default();
    let head = format!(
        "{method} /telegram HTTP/1.1\r\nHost: bot.example.com\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\n{secret}Connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(body).await.unwrap();
    let mut answer = Vec::new();
    let mut buf = [0u8; 1024];
    // A server that answers without reading the body may reset the connection after its answer.
    while let Ok(n) = stream.read(&mut buf).await {
        if n == 0 {
            break;
        }
        answer.extend_from_slice(&buf[..n]);
    }
    let answer = String::from_utf8_lossy(&answer);
    answer.split_whitespace().nth(1).and_then(|code| code.parse().ok()).expect("no status line")
}

#[cfg(feature = "webhook-server")]
#[tokio::test]
async fn the_built_in_server_checks_the_secret_and_hands_updates_on() {
    let stub = Stub::start().await;
    let (client, mut texts) = client(&stub.url).await;
    let webhook = client.webhook(SECRET).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = webhook.clone();
    let serve = tokio::spawn(async move { server.serve_listener(listener).await });

    let body = message(1, "hello");
    assert_eq!(status_of(addr, "POST", None, &body).await, 401);
    assert_eq!(status_of(addr, "POST", Some("wrong"), &body).await, 401);
    assert_eq!(status_of(addr, "GET", Some(SECRET), b"").await, 405);
    let taken = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match status_of(addr, "POST", Some(SECRET), &body).await {
                503 => tokio::time::sleep(Duration::from_millis(20)).await,
                status => return status,
            }
        }
    })
    .await
    .expect("the server never took the update");
    assert_eq!(taken, 200);
    assert_eq!(next_text(&mut texts).await, "hello");
    assert_eq!(status_of(addr, "POST", Some(SECRET), &body).await, 200);
    assert!(tokio::time::timeout(Duration::from_millis(300), texts.recv()).await.is_err(), "handled twice");

    client.stop();
    tokio::time::timeout(Duration::from_secs(10), serve).await.expect("serve didn't stop").unwrap().unwrap();
}
