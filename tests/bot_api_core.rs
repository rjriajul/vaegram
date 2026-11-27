//! A client in Bot API mode against a local HTTP stub: which Bot API method each friendly method sends and what
//! it returns, and a handler replying to a message `getUpdates` brought.
#![cfg(feature = "bot-api")]

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use vaegram::bot_api::Unsupported;
use vaegram::enums::ChatAction;
use vaegram::raw::{base, functions, types};
use vaegram::types::{BotCommand, InlineQueryResult};
use vaegram::{Client, filters};

const TOKEN: &str = "123:ABC";
const BOT: i64 = 99;
const USER: i64 = 7;
/// A basic group, Bot API style.
const GROUP: i64 = -42;
/// A supergroup, Bot API style, and its TL channel id.
const CHANNEL: i64 = -1_000_000_000_123;
const CHANNEL_ID: i64 = 123;

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
    let body = &data[head_end..head_end + length];
    let body = if length == 0 { json!({}) } else { serde_json::from_slice(body).unwrap() };
    let _ = calls.send((method.clone(), body.clone()));
    // An error is Telegram's envelope with `ok: false`.
    let reply = match answer(&method, &body, &pending).await {
        Ok(result) => json!({ "ok": true, "result": result }),
        Err(description) => json!({ "ok": false, "error_code": 400, "description": description }),
    }
    .to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

/// Message 51 is a media message: its text is a caption, which `editMessageText` refuses.
const MEDIA_MESSAGE: i64 = 51;

async fn answer(method: &str, body: &Value, pending: &Pending) -> Result<Value, &'static str> {
    let message_id = body["message_id"].as_i64().unwrap_or(0);
    Ok(match method {
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
        "editMessageText" if message_id == MEDIA_MESSAGE => {
            return Err("Bad Request: there is no text in the message to edit");
        }
        "editMessageText" | "editMessageCaption" | "editMessageReplyMarkup" => message(message_id, body),
        "forwardMessage" => message(60, body),
        "copyMessage" => json!({ "message_id": 61 }),
        _ => json!(true),
    })
}

/// A chat as `getChat` describes it: private for a positive id, a basic group above `-10^12`, else a supergroup.
fn chat(id: i64) -> Value {
    if id > 0 {
        json!({ "id": id, "type": "private", "first_name": "Ann", "bio": "hi there" })
    } else if id > -1_000_000_000_000 {
        json!({ "id": id, "type": "group", "title": "Group", "description": "about" })
    } else {
        json!({ "id": id, "type": "supergroup", "title": "Supergroup", "description": "about" })
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

    let commands =
        [BotCommand::new("start", "Start the bot"), BotCommand::new("status", "Check the bot").is_ephemeral(true)];
    client.set_bot_commands(commands).await.unwrap();
    let (method, body) = stub.next().await;
    assert_eq!(method, "setMyCommands");
    let sent = json!([
        { "command": "start", "description": "Start the bot" },
        { "command": "status", "description": "Check the bot", "is_ephemeral": true },
    ]);
    assert_eq!(body["commands"], sent);
    assert_eq!(body["scope"], json!({ "type": "default" }));

    // A private chat's message names no chat in TL: the bridge knows it from the send above.
    client.delete_messages(USER, &[50]).await.unwrap();
    let (method, body) = stub.next().await;
    assert_eq!(method, "deleteMessages");
    assert_eq!((&body["chat_id"], &body["message_ids"]), (&json!(USER), &json!([50])));

    // editMessageText refuses a media message's text, so the bridge edits its caption instead.
    let edited = client.edit_message_caption(USER, 51, "caption").await.unwrap();
    assert_eq!(edited.id, 51);
    let (method, body) = stub.next().await;
    assert_eq!((method.as_str(), &body["message_id"]), ("editMessageText", &json!(51)));
    let (method, body) = stub.next().await;
    assert_eq!(method, "editMessageCaption");
    let fields = (&body["chat_id"], &body["message_id"], &body["caption"]);
    assert_eq!(fields, (&json!(USER), &json!(51), &json!("caption")));

    let edited = client.edit_message_reply_markup(USER, 50, None).await.unwrap();
    assert_eq!(edited.id, 50);
    let (method, body) = stub.next().await;
    assert_eq!(method, "editMessageReplyMarkup");
    assert_eq!((&body["chat_id"], &body["message_id"]), (&json!(USER), &json!(50)));
    assert!(body.get("reply_markup").is_none() && body.get("text").is_none());

    let forwarded = client.forward_messages(USER, USER, &[50]).await.unwrap();
    assert_eq!(forwarded.iter().map(|m| m.id).collect::<Vec<_>>(), [60]);
    let (method, body) = stub.next().await;
    assert_eq!(method, "forwardMessage");
    let fields = (&body["chat_id"], &body["from_chat_id"], &body["message_id"]);
    assert_eq!(fields, (&json!(USER), &json!(USER), &json!(50)));

    // A copy through the TL forward with `drop_author`; the friendly `copy_message` takes its own branch to
    // `copyMessage` (tests/bot_api_media.rs).
    let user = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: USER, access_hash: 0 }));
    let copy = functions::messages::ForwardMessages {
        silent: false,
        background: false,
        with_my_score: false,
        drop_author: true,
        drop_media_captions: false,
        noforwards: false,
        allow_paid_floodskip: false,
        from_ephemeral: false,
        from_peer: user.clone(),
        id: vec![50],
        random_id: vec![1],
        to_peer: user,
        top_msg_id: None,
        reply_to: None,
        schedule_date: None,
        schedule_repeat_period: None,
        send_as: None,
        quick_reply_shortcut: None,
        effect: None,
        video_timestamp: None,
        allow_paid_stars: None,
        suggested_post: None,
    };
    client.invoke(&copy).await.unwrap();
    let (method, body) = stub.next().await;
    assert_eq!(method, "copyMessage");
    let fields = (&body["chat_id"], &body["from_chat_id"], &body["message_id"]);
    assert_eq!(fields, (&json!(USER), &json!(USER), &json!(50)));

    client.unpin_chat_message(USER, 50).await.unwrap();
    let (method, body) = stub.next().await;
    assert_eq!(method, "unpinChatMessage");
    assert_eq!((&body["chat_id"], &body["message_id"]), (&json!(USER), &json!(50)));

    client.unpin_all_chat_messages(USER).await.unwrap();
    let (method, body) = stub.next().await;
    assert_eq!((method.as_str(), &body["chat_id"]), ("unpinAllChatMessages", &json!(USER)));

    client.answer_inline_query(3, std::iter::empty::<InlineQueryResult>()).personal().await.unwrap();
    let (method, body) = stub.next().await;
    assert_eq!(method, "answerInlineQuery");
    assert_eq!((&body["inline_query_id"], &body["results"]), (&json!("3"), &json!([])));
    assert_eq!((&body["cache_time"], &body["is_personal"]), (&json!(300), &json!(true)));

    // users.getUsers for another user, and users.getFullUser: both are getChat.
    let users = client.get_users([8_i64]).await.unwrap();
    let user = users.into_iter().next().flatten().expect("user 8");
    assert_eq!((user.id, user.first_name.as_str()), (8, "Ann"));
    stub.next_where(get_chat(8)).await;
    let chat = client.get_chat(USER).await.unwrap();
    assert_eq!(chat.bio.as_deref(), Some("hi there"));
    stub.next_where(get_chat(USER)).await;

    // messages.getFullChat for a basic group; channels.getChannels, then channels.getFullChannel, for a supergroup.
    let chat = client.get_chat(GROUP).await.unwrap();
    assert_eq!((chat.title.as_deref(), chat.description.as_deref()), (Some("Group"), Some("about")));
    stub.next_where(get_chat(GROUP)).await;
    let chat = client.get_chat(CHANNEL).await.unwrap();
    assert_eq!((chat.title.as_deref(), chat.description.as_deref()), (Some("Supergroup"), Some("about")));
    stub.next_where(get_chat(CHANNEL)).await;
    stub.next_where(get_chat(CHANNEL)).await;

    // messages.getChats: no friendly method asks for a basic group by id alone.
    client.invoke(&functions::messages::GetChats { id: vec![-GROUP] }).await.unwrap();
    stub.next_where(get_chat(GROUP)).await;

    // A channel's messages are deleted through channels.deleteMessages, which names the channel.
    client.delete_messages(CHANNEL, &[5]).await.unwrap();
    let (method, body) = stub.next().await;
    assert_eq!(method, "deleteMessages");
    assert_eq!((&body["chat_id"], &body["message_ids"]), (&json!(CHANNEL), &json!([5])));

    // A forum topic's pins: no friendly method unpins all of one topic.
    let channel = types::InputPeerChannel { channel_id: CHANNEL_ID, access_hash: 0 };
    let peer = base::InputPeer::InputPeerChannel(Box::new(channel));
    let unpin = functions::messages::UnpinAllMessages { peer, top_msg_id: Some(3), saved_peer_id: None };
    client.invoke(&unpin).await.unwrap();
    let (method, body) = stub.next().await;
    assert_eq!(method, "unpinAllForumTopicMessages");
    assert_eq!((&body["chat_id"], &body["message_thread_id"]), (&json!(CHANNEL), &json!(3)));

    // A TL method with no Bot API counterpart fails without a request, naming the method.
    let error = client.invoke(&functions::help::GetConfig {}).await.unwrap_err();
    assert_eq!(error.downcast_ref::<Unsupported>().map(Unsupported::what), Some("help.getConfig"));
}

/// Matches the `getChat` that looks `id` up.
fn get_chat(id: i64) -> impl Fn(&Call) -> bool {
    move |(method, body): &Call| method == "getChat" && body["chat_id"] == json!(id)
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
