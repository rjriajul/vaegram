//! A router, its middleware and a provided value on a client in Bot API mode: updates from `getUpdates` reach the
//! same dispatcher as MTProto ones. The stub serves one batch of updates, then none.
#![cfg(feature = "bot-api")]

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use vaegram::handlers::{Dep, Next, Router, UpdateContext};
use vaegram::types::Message;
use vaegram::{Client, Error, filters};

const TOKEN: &str = "123:ABC";
const USER: i64 = 7;

type Pending = Arc<Mutex<Vec<Value>>>;

async fn serve(mut stream: TcpStream, pending: Pending) {
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
    let method = path.rsplit('/').next().unwrap_or_default();
    let result = match method {
        "getMe" => json!({ "id": 99, "is_bot": true, "first_name": "Bot", "username": "stub_bot" }),
        "getUpdates" => {
            let updates = std::mem::take(&mut *pending.lock().unwrap());
            if updates.is_empty() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Value::Array(updates)
        }
        _ => json!(true),
    };
    let reply = json!({ "ok": true, "result": result }).to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

fn message(update_id: i64, from: i64, text: &str) -> Value {
    let date = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    json!({
        "update_id": update_id,
        "message": {
            "message_id": update_id,
            "date": date,
            "chat": { "id": from, "type": "private", "first_name": "Ann" },
            "from": { "id": from, "is_bot": false, "first_name": "Ann" },
            "text": text
        }
    })
}

#[derive(Clone)]
struct Prefix(&'static str);

#[tokio::test]
async fn routers_route_bot_api_updates() {
    let pending: Pending = Arc::new(Mutex::new(vec![message(1, 8, "/start"), message(2, USER, "/start")]));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let shared = pending.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(serve(stream, shared.clone()));
        }
    });

    // `catch_up` keeps the stub's first batch, which a fresh poll would skip as a backlog.
    let client = Client::builder("routers_bot_api", 1, "hash")
        .bot_api(TOKEN)
        .bot_api_url(&url)
        .catch_up(true)
        .start()
        .await
        .unwrap();
    client.provide(Prefix("hello"));
    let (tx, mut seen) = mpsc::unbounded_channel();
    let mut router = Router::new();
    // Only USER gets through; the other user's update is kept from the handler.
    router.outer_middleware(|ctx: UpdateContext, next: Next| async move {
        if ctx.user_id() == Some(USER) { next.run(ctx).await } else { Ok(()) }
    });
    router.message().handle(filters::command("start"), move |msg: Message, Dep(prefix): Dep<Prefix>| {
        let tx = tx.clone();
        async move {
            let _ = tx.send(format!("{} {}", prefix.0, msg.chat.id));
            Ok::<_, Error>(())
        }
    });
    client.include_router(router);
    let runner = client.clone();
    let run = tokio::spawn(async move { runner.run().await });

    let got = tokio::time::timeout(Duration::from_secs(10), seen.recv()).await.expect("no update in time");
    assert_eq!(got.as_deref(), Some("hello 7"));
    client.stop();
    tokio::time::timeout(Duration::from_secs(10), run).await.expect("run didn't stop").unwrap().unwrap();
    assert!(seen.try_recv().is_err(), "the kept update reached the handler");
}
