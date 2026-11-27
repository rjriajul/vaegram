//! The bot's own methods for a client in Bot API mode, against a local HTTP stub: settings, managed bots, answers to
//! widget, guest and Mini App queries, business accounts, gifts, Stars, payments, stickers, stories and Passport.
//! Each check is one row: the friendly method, the Bot API method it sends with the fields that matter, and what it
//! reads back from the answer.
#![cfg(feature = "bot-api")]

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use vaegram::Client;
use vaegram::bot_api::Unsupported;
use vaegram::enums::{MaskPointType, PassportElementType, StickerType};
use vaegram::types::{
    BotCommandScope, ChatPrivileges, InlineQueryResult, InlineQueryResultArticle, InputInvoice, InputSticker,
    InputStoryContent, KeyboardButton, KeyboardButtonRequestManagedBot, LabeledPrice, MaskPosition, MenuButton,
    OwnedGift, PassportElementError, Upload,
};

const TOKEN: &str = "123:ABC";
const BOT: i64 = 99;
const USER: i64 = 7;
/// A bot the stub bot manages.
const MANAGED: i64 = 55;
/// A supergroup, Bot API style.
const CHANNEL: i64 = -1_000_000_000_123;
const CONNECTION: &str = "conn";
/// A document file id as the Bot API gives it out; stickers here use it too.
const DOCUMENT: &str = "BQACAgQAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAL6C87kooIPtf__________HgQ";
/// An inline message id: data center 2, id 1, access hash 2.
const INLINE_MESSAGE: &str = "AgAAAAEAAAAAAAAAAgAAAAAAAAA";
const SET: &str = "pack_by_stub_bot";

/// A file part of a multipart request: its field, file name and bytes.
#[derive(Debug, Clone, PartialEq)]
struct Part {
    field: String,
    filename: String,
    data: Vec<u8>,
}

/// One request the stub got: the Bot API method, its fields (strings for a multipart request), and its file parts.
#[derive(Debug)]
struct Call {
    method: String,
    body: Value,
    files: Vec<Part>,
}

struct Stub {
    url: String,
    calls: mpsc::UnboundedReceiver<Call>,
}

impl Stub {
    async fn start() -> Stub {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (tx, calls) = mpsc::unbounded_channel();
        let first_poll = Arc::new(Mutex::new(true));
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(serve(stream, tx.clone(), first_poll.clone()));
            }
        });
        Stub { url, calls }
    }

    /// The next request for `method`, past any other: lookups the client makes on its own, and the calls a method
    /// makes at once, which come in any order.
    async fn call(&mut self, method: &str) -> Call {
        loop {
            let call = tokio::time::timeout(Duration::from_secs(10), self.calls.recv())
                .await
                .unwrap_or_else(|_| panic!("no {method} request in time"))
                .expect("the stub stopped");
            if call.method == method {
                return call;
            }
        }
    }

    /// Whether any request came in since the last one read, after a moment for one in flight.
    async fn idle(&mut self) -> bool {
        tokio::time::sleep(Duration::from_millis(50)).await;
        loop {
            match self.calls.try_recv() {
                Ok(call) if matches!(call.method.as_str(), "getMe" | "getChat" | "getUpdates") => {}
                Ok(_) => return false,
                Err(_) => return true,
            }
        }
    }
}

async fn serve(mut stream: TcpStream, calls: mpsc::UnboundedSender<Call>, first_poll: Arc<Mutex<bool>>) {
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
    let header = |wanted: &str| {
        head.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case(wanted).then(|| value.trim().to_owned())
        })
    };
    let length = header("content-length").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
    let content_type = header("content-type").unwrap_or_default();
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
    let raw = &data[head_end..head_end + length];
    let (body, files) = match content_type.split_once("boundary=") {
        Some((_, boundary)) => multipart(raw, boundary),
        None if length == 0 => (json!({}), Vec::new()),
        None => (serde_json::from_slice(raw).unwrap(), Vec::new()),
    };
    if method == "getUpdates" {
        let first = std::mem::replace(&mut *first_poll.lock().unwrap(), false);
        if !first {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    let _ = calls.send(Call { method: method.clone(), body: body.clone(), files });
    let reply = json!({ "ok": true, "result": answer(&method, &body) }).to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

/// The text fields of a `multipart/form-data` body as a JSON object of strings, and its file parts.
fn multipart(body: &[u8], boundary: &str) -> (Value, Vec<Part>) {
    let delimiter = format!("--{}", boundary.trim_matches('"'));
    let mut fields = serde_json::Map::new();
    let mut files = Vec::new();
    for part in split(body, delimiter.as_bytes()).into_iter().skip(1) {
        if part.starts_with(b"--") {
            break;
        }
        let part = part.strip_prefix(b"\r\n").unwrap_or(part);
        let part = part.strip_suffix(b"\r\n").unwrap_or(part);
        let end = part.windows(4).position(|w| w == b"\r\n\r\n").expect("a part without headers");
        let head = String::from_utf8_lossy(&part[..end]).into_owned();
        let content = &part[end + 4..];
        let name = parameter(&head, "name").expect("a part without a name");
        match parameter(&head, "filename") {
            Some(filename) => files.push(Part { field: name, filename, data: content.to_vec() }),
            None => {
                fields.insert(name, Value::String(String::from_utf8_lossy(content).into_owned()));
            }
        }
    }
    (Value::Object(fields), files)
}

fn split<'a>(data: &'a [u8], separator: &[u8]) -> Vec<&'a [u8]> {
    let mut parts = Vec::new();
    let mut rest = data;
    while let Some(i) = rest.windows(separator.len()).position(|w| w == separator) {
        parts.push(&rest[..i]);
        rest = &rest[i + separator.len()..];
    }
    parts.push(rest);
    parts
}

/// `key="value"` of a part's `Content-Disposition`.
fn parameter(head: &str, key: &str) -> Option<String> {
    let marker = format!("; {key}=\"");
    let start = head.find(&marker)? + marker.len();
    let end = head[start..].find('"')? + start;
    Some(head[start..end].to_owned())
}

/// A JSON field a multipart request sent as text, read back as JSON.
fn field(body: &Value, key: &str) -> Value {
    serde_json::from_str(body[key].as_str().unwrap_or_else(|| panic!("{key} as a field"))).unwrap()
}

fn answer(method: &str, body: &Value) -> Value {
    match method {
        "getMe" => json!({ "id": BOT, "is_bot": true, "first_name": "Bot", "username": "stub_bot" }),
        "getChat" => chat(body["chat_id"].as_i64().unwrap_or(USER)),
        "getUpdates" => json!([]),
        "answerGuestQuery" | "answerWebAppQuery" => json!({ "inline_message_id": INLINE_MESSAGE }),
        "savePreparedInlineMessage" => json!({ "id": "prepared", "expiration_date": 1_900_000_000 }),
        "savePreparedKeyboardButton" => json!({ "id": "button" }),
        "getManagedBotToken" => json!("777:OLD"),
        "replaceManagedBotToken" => json!("777:NEW"),
        "getManagedBotAccessSettings" => json!({
            "is_access_restricted": true,
            "added_users": [{ "id": 8, "is_bot": false, "first_name": "Bo" }],
        }),
        "getMyCommands" => json!([{ "command": "start", "description": "Start the bot" }]),
        "getMyName" => json!({ "name": "Stub name" }),
        "getMyDescription" => json!({ "description": "Stub description" }),
        "getMyShortDescription" => json!({ "short_description": "Stub about" }),
        "getChatMenuButton" => {
            json!({ "type": "web_app", "text": "Open", "web_app": { "url": "https://example.com/app" } })
        }
        "getMyDefaultAdministratorRights" => json!({
            "is_anonymous": false,
            "can_manage_chat": true,
            "can_delete_messages": true,
            "can_manage_video_chats": false,
            "can_restrict_members": false,
            "can_promote_members": false,
            "can_change_info": false,
            "can_invite_users": false,
            "can_post_stories": false,
            "can_edit_stories": false,
            "can_delete_stories": false,
            "can_send_welcome_messages": false,
        }),
        "getBusinessConnection" => json!({
            "id": CONNECTION,
            "user": { "id": USER, "is_bot": false, "first_name": "Ann" },
            "user_chat_id": USER,
            "date": 1_700_000_000,
            "rights": { "can_reply": true, "can_view_gifts_and_stars": true },
            "is_enabled": true,
        }),
        "getBusinessAccountStarBalance" => json!({ "amount": 50, "nanostar_amount": 5 }),
        "getMyStarBalance" => json!({ "amount": 100 }),
        "getStarTransactions" => json!({
            "transactions": [
                {
                    "id": "t1",
                    "amount": 10,
                    "date": 1_700_000_000,
                    "source": {
                        "type": "user",
                        "transaction_type": "invoice_payment",
                        "user": { "id": USER, "is_bot": false, "first_name": "Ann" },
                        "invoice_payload": "order-1",
                    },
                },
                { "id": "t2", "amount": 5, "date": 1_700_000_001, "receiver": { "type": "fragment" } },
            ],
        }),
        "getAvailableGifts" => json!({ "gifts": [gift()] }),
        "getUserGifts" | "getChatGifts" | "getBusinessAccountGifts" => json!({
            "total_count": 1,
            "gifts": [{
                "type": "regular",
                "gift": gift(),
                "owned_gift_id": "42",
                "sender_user": { "id": USER, "is_bot": false, "first_name": "Ann" },
                "send_date": 1_700_000_000,
                "text": "for you",
                "is_saved": true,
            }],
        }),
        "getUniqueGift" => json!({
            "gift_id": "5",
            "base_name": "Pepe",
            "name": body["name"],
            "number": 3,
            "model": { "name": "Model", "sticker": sticker("regular"), "rarity_per_mille": 10 },
            "symbol": { "name": "Symbol", "sticker": sticker("regular"), "rarity_per_mille": 20 },
            "backdrop": {
                "name": "Backdrop",
                "colors": { "center_color": 1, "edge_color": 2, "symbol_color": 3, "text_color": 4 },
                "rarity_per_mille": 30,
            },
        }),
        "getStickerSet" => json!({
            "name": body["name"],
            "title": "Pack",
            "sticker_type": "regular",
            "stickers": [sticker("regular")],
        }),
        "getCustomEmojiStickers" => json!([sticker("custom_emoji")]),
        "uploadStickerFile" => json!({ "file_id": DOCUMENT, "file_unique_id": "u", "file_size": 4 }),
        "createInvoiceLink" => json!("https://t.me/$invoice"),
        "sendInvoice" => json!({
            "message_id": 80,
            "date": unix_now(),
            "chat": chat(body["chat_id"].as_i64().unwrap_or(USER)),
            "from": { "id": BOT, "is_bot": true, "first_name": "Bot" },
        }),
        "postStory" | "repostStory" | "editStory" => json!({ "chat": chat(USER), "id": 9 }),
        _ => json!(true),
    }
}

/// A chat as `getChat` describes it: private for a positive id, else a supergroup.
fn chat(id: i64) -> Value {
    if id > 0 {
        json!({ "id": id, "type": "private", "first_name": "Ann" })
    } else {
        json!({ "id": id, "type": "supergroup", "title": "Supergroup" })
    }
}

fn sticker(kind: &str) -> Value {
    json!({
        "file_id": DOCUMENT,
        "file_unique_id": "s",
        "type": kind,
        "width": 512,
        "height": 512,
        "is_animated": false,
        "is_video": false,
        "emoji": "😀",
        "set_name": SET,
        "file_size": 100,
    })
}

fn gift() -> Value {
    json!({ "id": "5", "sticker": sticker("regular"), "star_count": 25, "total_count": 10, "remaining_count": 3 })
}

fn article() -> InlineQueryResult {
    InlineQueryResult::from(InlineQueryResultArticle::new("Hello", "hello there"))
}

fn invoice() -> InputInvoice {
    InputInvoice::new("Pro", "One month of Pro", "order-1", "XTR", [LabeledPrice::new("Pro", 100)])
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

async fn client(url: &str) -> Client {
    Client::builder("bot_api_bots", 1, "hash").bot_api(TOKEN).bot_api_url(url).catch_up(true).start().await.unwrap()
}

/// The part of the Bot API an error says the request has none of, if it is [`Unsupported`].
fn unsupported(error: &vaegram::Error) -> Option<&'static str> {
    error.downcast_ref::<Unsupported>().map(Unsupported::what)
}

#[tokio::test]
async fn bot_settings_and_answers() {
    let mut stub = Stub::start().await;
    let client = client(&stub.url).await;

    client.answer_widget_query(5).data("ok").await.unwrap();
    let call = stub.call("answerWidgetQuery").await;
    assert_eq!((&call.body["widget_query_id"], &call.body["data"]), (&json!("5"), &json!("ok")));

    client.answer_guest_query(6, article()).await.unwrap();
    let call = stub.call("answerGuestQuery").await;
    assert_eq!((&call.body["guest_query_id"], &call.body["result"]["type"]), (&json!("6"), &json!("article")));

    // The inline message id comes back decoded, as TL's answer carries it.
    let sent = client.answer_web_app_query("wq", article()).await.unwrap();
    assert_eq!(sent.map(|id| id.to_string()).as_deref(), Some(INLINE_MESSAGE));
    let call = stub.call("answerWebAppQuery").await;
    assert_eq!(call.body["web_app_query_id"], json!("wq"));

    let prepared = client.save_prepared_inline_message(USER, article()).allow_user_chats().await.unwrap();
    assert_eq!((prepared.id.as_str(), prepared.expiration_date), ("prepared", 1_900_000_000));
    let call = stub.call("savePreparedInlineMessage").await;
    assert_eq!((&call.body["user_id"], &call.body["allow_user_chats"]), (&json!(USER), &json!(true)));
    assert!(call.body.get("allow_bot_chats").is_none());

    let request = KeyboardButtonRequestManagedBot::new(4).suggested_name("Helper");
    let button = KeyboardButton::new("New bot").request_managed_bot(request);
    assert_eq!(client.save_prepared_keyboard_button(USER, &button).await.unwrap(), "button");
    let call = stub.call("savePreparedKeyboardButton").await;
    let expected = json!({ "text": "New bot", "request_managed_bot": { "request_id": 4, "suggested_name": "Helper" } });
    assert_eq!((&call.body["user_id"], &call.body["button"]), (&json!(USER), &expected));

    assert_eq!(client.get_managed_bot_token(MANAGED).await.unwrap(), "777:OLD");
    assert_eq!(stub.call("getManagedBotToken").await.body["user_id"], json!(MANAGED));
    assert_eq!(client.replace_managed_bot_token(MANAGED).await.unwrap(), "777:NEW");
    assert_eq!(stub.call("replaceManagedBotToken").await.body["user_id"], json!(MANAGED));

    let access = client.get_managed_bot_access_settings(MANAGED).await.unwrap();
    assert!(access.is_access_restricted);
    assert_eq!(access.added_users.iter().map(|u| u.id).collect::<Vec<_>>(), [8]);
    stub.call("getManagedBotAccessSettings").await;
    client.set_managed_bot_access_settings(MANAGED).restrict_to([USER]).await.unwrap();
    let call = stub.call("setManagedBotAccessSettings").await;
    let fields = (&call.body["user_id"], &call.body["is_access_restricted"], &call.body["added_user_ids"]);
    assert_eq!(fields, (&json!(MANAGED), &json!(true), &json!([USER])));

    client.delete_bot_commands().scope(BotCommandScope::AllPrivateChats).language_code("en").await.unwrap();
    let call = stub.call("deleteMyCommands").await;
    assert_eq!(call.body, json!({ "scope": { "type": "all_private_chats" }, "language_code": "en" }));
    let commands = client.get_bot_commands().await.unwrap();
    assert_eq!(commands.iter().map(|c| c.command.as_str()).collect::<Vec<_>>(), ["start"]);
    assert_eq!(stub.call("getMyCommands").await.body["scope"], json!({ "type": "default" }));

    client.set_bot_name("Stub name").language_code("en").await.unwrap();
    assert_eq!(stub.call("setMyName").await.body, json!({ "name": "Stub name", "language_code": "en" }));
    client.set_bot_description("Stub description").await.unwrap();
    assert_eq!(stub.call("setMyDescription").await.body, json!({ "description": "Stub description" }));
    client.set_bot_short_description("Stub about").await.unwrap();
    assert_eq!(stub.call("setMyShortDescription").await.body, json!({ "short_description": "Stub about" }));
    // TL reads the name and both descriptions at once; each getter returns its own.
    assert_eq!(client.get_bot_name().await.unwrap(), "Stub name");
    assert_eq!(client.get_bot_description().await.unwrap(), "Stub description");
    assert_eq!(client.get_bot_short_description().await.unwrap(), "Stub about");

    let menu = MenuButton::web_app("Open", "https://example.com/app");
    client.set_chat_menu_button(Some(USER.into()), menu.clone()).await.unwrap();
    let call = stub.call("setChatMenuButton").await;
    let expected = json!({ "type": "web_app", "text": "Open", "web_app": { "url": "https://example.com/app" } });
    assert_eq!((&call.body["chat_id"], &call.body["menu_button"]), (&json!(USER), &expected));
    client.set_chat_menu_button(None, MenuButton::Commands).await.unwrap();
    assert_eq!(stub.call("setChatMenuButton").await.body, json!({ "menu_button": { "type": "commands" } }));
    assert_eq!(client.get_chat_menu_button(None).await.unwrap(), menu);
    assert_eq!(stub.call("getChatMenuButton").await.body, json!({}));

    let privileges = ChatPrivileges::new().can_manage_chat(true).can_delete_messages(true);
    client.set_bot_default_privileges(&privileges, true).await.unwrap();
    let call = stub.call("setMyDefaultAdministratorRights").await;
    assert_eq!(call.body["for_channels"], json!(true));
    let rights = &call.body["rights"];
    assert_eq!((&rights["can_manage_chat"], &rights["can_post_messages"]), (&json!(true), &json!(false)));
    let read = client.get_bot_default_privileges(false).await.unwrap().expect("default rights");
    assert!(read.can_manage_chat && read.can_delete_messages && !read.can_promote_members);
    assert_eq!(stub.call("getMyDefaultAdministratorRights").await.body, json!({ "for_channels": false }));

    // A profile photo goes in the request that sets it, as an `InputProfilePhoto` naming its part.
    client.set_profile_photo(Upload::bytes("me.jpg", b"jpeg")).await.unwrap();
    let call = stub.call("setMyProfilePhoto").await;
    assert_eq!(field(&call.body, "photo"), json!({ "type": "static", "photo": "attach://file0" }));
    assert_eq!((call.files[0].field.as_str(), call.files[0].data.as_slice()), ("file0", b"jpeg".as_slice()));
    client.set_profile_photo(Upload::bytes("me.mp4", b"mp4")).video_start_ts(1.5).await.unwrap();
    let call = stub.call("setMyProfilePhoto").await;
    let expected = json!({ "type": "animated", "animation": "attach://file0", "main_frame_timestamp": 1.5 });
    assert_eq!(field(&call.body, "photo"), expected);
    client.remove_profile_photo().await.unwrap();
    stub.call("removeMyProfilePhoto").await;

    // The Bot API sets only the bot's own main photo.
    let error = client.set_profile_photo(Upload::bytes("me.jpg", b"jpeg")).public().await.unwrap_err();
    assert_eq!(unsupported(&error), Some("the public profile photo of a bot"));
    assert!(stub.idle().await);
}

#[tokio::test]
async fn business_accounts() {
    let mut stub = Stub::start().await;
    let client = client(&stub.url).await;

    let connection = client.get_business_connection(CONNECTION).await.unwrap();
    assert_eq!((connection.id.as_str(), connection.user.id, connection.is_enabled), (CONNECTION, USER, true));
    assert!(
        connection.rights.can_reply && connection.rights.can_view_gifts_and_stars && !connection.rights.can_edit_bio
    );
    assert_eq!(stub.call("getBusinessConnection").await.body, json!({ "business_connection_id": CONNECTION }));

    client.read_business_message(CONNECTION, USER, 50).await.unwrap();
    let call = stub.call("readBusinessMessage").await;
    let expected = json!({ "business_connection_id": CONNECTION, "chat_id": USER, "message_id": 50 });
    assert_eq!(call.body, expected);

    client.delete_business_messages(CONNECTION, &[1, 2]).await.unwrap();
    let call = stub.call("deleteBusinessMessages").await;
    assert_eq!(call.body, json!({ "business_connection_id": CONNECTION, "message_ids": [1, 2] }));

    client.set_business_account_name(CONNECTION, "Ann", "Lee").await.unwrap();
    let call = stub.call("setBusinessAccountName").await;
    assert_eq!(call.body, json!({ "business_connection_id": CONNECTION, "first_name": "Ann", "last_name": "Lee" }));
    client.set_business_account_username(CONNECTION, "ann").await.unwrap();
    let call = stub.call("setBusinessAccountUsername").await;
    assert_eq!(call.body, json!({ "business_connection_id": CONNECTION, "username": "ann" }));
    client.set_business_account_bio(CONNECTION, "Shop owner").await.unwrap();
    let call = stub.call("setBusinessAccountBio").await;
    assert_eq!(call.body, json!({ "business_connection_id": CONNECTION, "bio": "Shop owner" }));

    client.set_business_account_profile_photo(CONNECTION, Upload::bytes("shop.jpg", b"jpeg")).public().await.unwrap();
    let call = stub.call("setBusinessAccountProfilePhoto").await;
    assert_eq!((&call.body["business_connection_id"], &call.body["is_public"]), (&json!(CONNECTION), &json!("true")));
    assert_eq!(field(&call.body, "photo"), json!({ "type": "static", "photo": "attach://file0" }));
    assert_eq!(call.files[0].filename, "shop.jpg");
    client.remove_business_account_profile_photo(CONNECTION, true).await.unwrap();
    let call = stub.call("removeBusinessAccountProfilePhoto").await;
    assert_eq!(call.body, json!({ "business_connection_id": CONNECTION, "is_public": true }));

    // The Bot API can't read the settings: a setting left unset goes as its default.
    client.set_business_account_gift_settings(CONNECTION).show_gift_button(true).await.unwrap();
    let call = stub.call("setBusinessAccountGiftSettings").await;
    assert_eq!(call.body["show_gift_button"], json!(true));
    let all = json!({
        "unlimited_gifts": true,
        "limited_gifts": true,
        "unique_gifts": true,
        "premium_subscription": true,
        "gifts_from_channels": true,
    });
    assert_eq!(call.body["accepted_gift_types"], all);

    let balance = client.get_business_account_star_balance(CONNECTION).await.unwrap();
    assert_eq!((balance.amount, balance.nanostar_amount), (50, 5));
    stub.call("getBusinessAccountStarBalance").await;
    client.transfer_business_account_stars(CONNECTION, 30).await.unwrap();
    let call = stub.call("transferBusinessAccountStars").await;
    assert_eq!(call.body, json!({ "business_connection_id": CONNECTION, "star_count": 30 }));

    let gifts = client.get_business_account_gifts(CONNECTION).limit(5).await.unwrap();
    let ids: Vec<_> = gifts
        .iter()
        .map(|g| match g {
            OwnedGift::Regular(g) => g.owned_gift_id.clone(),
            _ => None,
        })
        .collect();
    assert_eq!(ids, [Some("42".to_owned())]);
    let call = stub.call("getBusinessAccountGifts").await;
    assert_eq!(call.body["business_connection_id"], json!(CONNECTION));

    client.convert_gift_to_stars("42").business_connection_id(CONNECTION).await.unwrap();
    let call = stub.call("convertGiftToStars").await;
    assert_eq!(call.body, json!({ "business_connection_id": CONNECTION, "owned_gift_id": "42" }));
    client.upgrade_gift("42").business_connection_id(CONNECTION).keep_original_details().await.unwrap();
    let call = stub.call("upgradeGift").await;
    let expected =
        json!({ "business_connection_id": CONNECTION, "owned_gift_id": "42", "keep_original_details": true });
    assert_eq!(call.body, expected);
    // A paid upgrade goes through TL's payment form, which the bridge pays in the one `upgradeGift` call.
    client.upgrade_gift("PlushPepe-3").business_connection_id(CONNECTION).pay_for_upgrade().await.unwrap();
    assert_eq!(stub.call("upgradeGift").await.body["owned_gift_id"], json!("PlushPepe-3"));
    client.transfer_gift("42", USER).business_connection_id(CONNECTION).await.unwrap();
    let call = stub.call("transferGift").await;
    let expected = json!({ "business_connection_id": CONNECTION, "owned_gift_id": "42", "new_owner_chat_id": USER });
    assert_eq!(call.body, expected);
    client.delete_business_account_gift(CONNECTION, "42").await.unwrap();
    let call = stub.call("deleteBusinessAccountGift").await;
    assert_eq!(call.body, json!({ "business_connection_id": CONNECTION, "owned_gift_id": "42" }));

    // Without a connection the Bot API has nothing to convert: the bot's own gifts aren't its to give up.
    let error = client.convert_gift_to_stars("42").await.unwrap_err();
    assert_eq!(unsupported(&error), Some("converting, upgrading, transferring or deleting the bot's own gifts"));
    assert!(stub.idle().await);
}

#[tokio::test]
async fn business_stories() {
    let mut stub = Stub::start().await;
    let client = client(&stub.url).await;

    let content = InputStoryContent::photo(Upload::bytes("story.jpg", b"jpeg"));
    let story =
        client.post_business_story(CONNECTION, content, 86_400).caption("hello").post_to_chat_page().await.unwrap();
    assert_eq!((story.id, story.chat.id, story.caption.as_deref()), (9, USER, Some("hello")));
    assert!(story.is_pinned && story.expire_date > story.date);
    let call = stub.call("postStory").await;
    let fields = (&call.body["business_connection_id"], &call.body["active_period"], &call.body["caption"]);
    assert_eq!(fields, (&json!(CONNECTION), &json!("86400"), &json!("hello")));
    assert_eq!(call.body["post_to_chat_page"], json!("true"));
    assert_eq!(field(&call.body, "content"), json!({ "type": "photo", "photo": "attach://file0" }));
    assert_eq!(call.files[0].filename, "story.jpg");

    let story = client.repost_business_story(CONNECTION, USER, 3, 86_400).await.unwrap();
    assert_eq!((story.id, story.forward_from_story_id), (9, Some(3)));
    let call = stub.call("repostStory").await;
    let expected = json!({
        "business_connection_id": CONNECTION,
        "from_chat_id": USER,
        "from_story_id": 3,
        "active_period": 86_400,
    });
    assert_eq!(call.body, expected);

    let video = InputStoryContent::video(Upload::bytes("story.mp4", b"mp4"), 5.0);
    let story = client.edit_business_story(CONNECTION, 9, video).await.unwrap();
    assert!(story.is_edited);
    let call = stub.call("editStory").await;
    assert_eq!(call.body["story_id"], json!("9"));
    let content = field(&call.body, "content");
    assert_eq!(
        (&content["type"], &content["video"], &content["duration"]),
        (&json!("video"), &json!("attach://file0"), &json!(5.0))
    );

    client.delete_business_story(CONNECTION, 9).await.unwrap();
    let call = stub.call("deleteStory").await;
    assert_eq!(call.body, json!({ "business_connection_id": CONNECTION, "story_id": 9 }));
}

#[tokio::test]
async fn gifts_stars_and_payments() {
    let mut stub = Stub::start().await;
    let client = client(&stub.url).await;

    let gifts = client.get_available_gifts().await.unwrap();
    assert_eq!(gifts.iter().map(|g| (g.id, g.star_count, g.total_count)).collect::<Vec<_>>(), [(5, 25, Some(10))]);
    stub.call("getAvailableGifts").await;

    // A Stars payment is TL's form and then its payment; the Bot API pays in one call.
    client.send_gift(USER, 5).text("for you").pay_for_upgrade().await.unwrap();
    let call = stub.call("sendGift").await;
    let expected = json!({ "user_id": USER, "gift_id": "5", "pay_for_upgrade": true, "text": "for you" });
    assert_eq!(call.body, expected);
    client.gift_premium_subscription(USER, 6).await.unwrap();
    let call = stub.call("giftPremiumSubscription").await;
    assert_eq!(call.body, json!({ "user_id": USER, "month_count": 6, "star_count": 1500 }));
    // The Bot API prices Premium only for 3, 6 and 12 months; any other length fails before a request goes out.
    let error = client.gift_premium_subscription(USER, 4).await.unwrap_err();
    assert!(matches!(error, vaegram::Error::InvalidArgument(_)), "{error:?}");
    let error = client.send_gift(USER, 5).private().await.unwrap_err();
    assert_eq!(unsupported(&error), Some("gifts that hide the sender's name"));

    let gifts = client.get_user_gifts(USER).limit(5).await.unwrap();
    assert_eq!(gifts.len(), 1);
    assert_eq!(stub.call("getUserGifts").await.body["user_id"], json!(USER));
    let gifts = client.get_chat_gifts(CHANNEL).exclude_unsaved().limit(5).await.unwrap();
    assert_eq!(gifts.len(), 1);
    let call = stub.call("getChatGifts").await;
    assert_eq!((&call.body["chat_id"], &call.body["exclude_unsaved"]), (&json!(CHANNEL), &json!(true)));

    let unique = client.get_unique_gift("PlushPepe-3").await.unwrap();
    assert_eq!((unique.name.as_str(), unique.number, unique.gift_id), ("PlushPepe-3", 3, 5));
    assert_eq!(unique.model.map(|m| m.name).as_deref(), Some("Model"));
    assert_eq!(stub.call("getUniqueGift").await.body, json!({ "name": "PlushPepe-3" }));

    let balance = client.get_my_star_balance().await.unwrap();
    assert_eq!(balance.amount, 100);
    stub.call("getMyStarBalance").await;

    // Money in comes from its source, money out goes to its receiver and counts negative.
    let transactions = client.get_star_transactions().limit(10).await.unwrap();
    let amounts: Vec<_> = transactions.iter().map(|t| (t.id.as_str(), t.amount.amount)).collect();
    assert_eq!(amounts, [("t1", 10), ("t2", -5)]);
    assert_eq!(stub.call("getStarTransactions").await.body, json!({ "offset": 0, "limit": 10 }));

    client.refund_star_payment(USER, "charge-1").await.unwrap();
    let call = stub.call("refundStarPayment").await;
    assert_eq!(call.body, json!({ "user_id": USER, "telegram_payment_charge_id": "charge-1" }));
    client.edit_user_star_subscription(USER, "charge-2", true).await.unwrap();
    let call = stub.call("editUserStarSubscription").await;
    let expected = json!({ "user_id": USER, "telegram_payment_charge_id": "charge-2", "is_canceled": true });
    assert_eq!(call.body, expected);

    let sent = client.send_invoice(USER, invoice()).await.unwrap();
    assert_eq!((sent.id, sent.chat.id), (80, USER));
    let call = stub.call("sendInvoice").await;
    let fields = (&call.body["chat_id"], &call.body["title"], &call.body["payload"], &call.body["currency"]);
    assert_eq!(fields, (&json!(USER), &json!("Pro"), &json!("order-1"), &json!("XTR")));
    assert_eq!(call.body["prices"], json!([{ "label": "Pro", "amount": 100 }]));
    assert!(call.body.get("provider_data").is_none() && call.body.get("provider_token").is_none());

    let link = client.create_invoice_link(invoice()).business_connection_id(CONNECTION).await.unwrap();
    assert_eq!(link, "https://t.me/$invoice");
    let call = stub.call("createInvoiceLink").await;
    assert_eq!(
        (&call.body["business_connection_id"], &call.body["description"]),
        (&json!(CONNECTION), &json!("One month of Pro"))
    );

    client.answer_shipping_query(7).error("We don't ship there").await.unwrap();
    let call = stub.call("answerShippingQuery").await;
    let expected = json!({ "shipping_query_id": "7", "ok": false, "error_message": "We don't ship there" });
    assert_eq!(call.body, expected);
    client.answer_pre_checkout_query(8).await.unwrap();
    let call = stub.call("answerPreCheckoutQuery").await;
    assert_eq!(call.body, json!({ "pre_checkout_query_id": "8", "ok": true }));

    let error = PassportElementError::Files {
        kind: PassportElementType::UtilityBill,
        file_hashes: vec![vec![1, 2, 3]],
        message: "The bill is blurry".into(),
    };
    client.set_passport_data_errors(USER, [error]).await.unwrap();
    let call = stub.call("setPassportDataErrors").await;
    let expected = json!([{
        "source": "files",
        "type": "utility_bill",
        "file_hashes": ["AQID"],
        "message": "The bill is blurry",
    }]);
    assert_eq!((&call.body["user_id"], &call.body["errors"]), (&json!(USER), &expected));
}

#[tokio::test]
async fn stickers_and_sets() {
    let mut stub = Stub::start().await;
    let client = client(&stub.url).await;

    let set = client.get_sticker_set(SET).await.unwrap();
    assert_eq!((set.name.as_str(), set.title.as_str(), set.sticker_type), (SET, "Pack", StickerType::Regular));
    let sticker = set.stickers.first().cloned().expect("a sticker");
    assert_eq!((sticker.set_name.as_deref(), sticker.emojis.as_slice()), (Some(SET), ["😀".to_owned()].as_slice()));
    assert_eq!(stub.call("getStickerSet").await.body, json!({ "name": SET }));

    let emoji = client.get_custom_emoji_stickers(&[777]).await.unwrap();
    assert_eq!(emoji.iter().map(|s| s.kind).collect::<Vec<_>>(), [StickerType::CustomEmoji]);
    assert_eq!(stub.call("getCustomEmojiStickers").await.body, json!({ "custom_emoji_ids": ["777"] }));

    let document = client.upload_sticker_file(USER, Upload::bytes("cat.webp", b"webp")).await.unwrap();
    assert!(!document.file_id.is_empty());
    let call = stub.call("uploadStickerFile").await;
    assert_eq!((&call.body["user_id"], &call.body["sticker_format"]), (&json!("7"), &json!("static")));
    // A file that is a whole field goes as that field's part.
    assert_eq!((call.files[0].field.as_str(), call.files[0].filename.as_str()), ("sticker", "cat.webp"));

    // Every new file goes in the one request, each named by its part; the set is read back after.
    let new = InputSticker::new(Upload::bytes("dance.tgs", b"tgs"), "😀👍").keywords(["dance", "fun"]);
    let created = client.create_new_sticker_set(USER, SET, "Pack", [new]).await.unwrap();
    assert_eq!(created.name, SET);
    let call = stub.call("createNewStickerSet").await;
    assert_eq!(
        (&call.body["name"], &call.body["title"], &call.body["sticker_type"]),
        (&json!(SET), &json!("Pack"), &json!("regular"))
    );
    let expected = json!([{
        "sticker": "attach://file0",
        "format": "animated",
        "emoji_list": ["😀", "👍"],
        "keywords": ["dance", "fun"],
    }]);
    assert_eq!(field(&call.body, "stickers"), expected);
    stub.call("getStickerSet").await;

    // A sticker already on Telegram goes by its file id, with no upload.
    client.add_sticker_to_set(USER, SET, InputSticker::existing(&sticker, "😀")).await.unwrap();
    let call = stub.call("addStickerToSet").await;
    assert!(call.files.is_empty());
    assert_eq!((&call.body["user_id"], &call.body["name"]), (&json!(USER), &json!(SET)));
    assert_eq!(
        (&call.body["sticker"]["format"], &call.body["sticker"]["emoji_list"]),
        (&json!("static"), &json!(["😀"]))
    );
    assert!(call.body["sticker"]["sticker"].as_str().is_some_and(|id| !id.is_empty()));

    let new = InputSticker::new(Upload::bytes("new.webm", b"webm"), "🔥");
    client.replace_sticker_in_set(USER, &sticker, new).await.unwrap();
    let call = stub.call("replaceStickerInSet").await;
    assert_eq!((&call.body["name"], &call.body["old_sticker"]), (&json!(SET), &json!(sticker.file_id)));
    assert_eq!(field(&call.body, "sticker")["format"], json!("video"));

    client.set_sticker_position_in_set(&sticker, 0).await.unwrap();
    let call = stub.call("setStickerPositionInSet").await;
    assert_eq!(call.body, json!({ "sticker": sticker.file_id, "position": 0 }));
    client.delete_sticker_from_set(&sticker).await.unwrap();
    assert_eq!(stub.call("deleteStickerFromSet").await.body, json!({ "sticker": sticker.file_id }));
    client.set_sticker_emoji_list(&sticker, "😀😎").await.unwrap();
    let call = stub.call("setStickerEmojiList").await;
    assert_eq!(call.body, json!({ "sticker": sticker.file_id, "emoji_list": ["😀", "😎"] }));
    client.set_sticker_keywords(&sticker, ["cat", "cute"]).await.unwrap();
    let call = stub.call("setStickerKeywords").await;
    assert_eq!(call.body, json!({ "sticker": sticker.file_id, "keywords": ["cat", "cute"] }));
    client.set_sticker_mask_position(&sticker, MaskPosition::new(MaskPointType::Eyes, 0.5, -0.25, 1.5)).await.unwrap();
    let call = stub.call("setStickerMaskPosition").await;
    let expected = json!({ "point": "eyes", "x_shift": 0.5, "y_shift": -0.25, "scale": 1.5 });
    assert_eq!(call.body["mask_position"], expected);
    client.remove_sticker_mask_position(&sticker).await.unwrap();
    assert_eq!(stub.call("setStickerMaskPosition").await.body, json!({ "sticker": sticker.file_id }));

    client.set_sticker_set_title(SET, "New title").await.unwrap();
    assert_eq!(stub.call("setStickerSetTitle").await.body, json!({ "name": SET, "title": "New title" }));
    client.set_sticker_set_thumbnail(SET, USER, Some(Upload::bytes("thumb.webp", b"webp"))).await.unwrap();
    let call = stub.call("setStickerSetThumbnail").await;
    let fields = (&call.body["name"], &call.body["format"], &call.body["thumbnail"]);
    assert_eq!(fields, (&json!(SET), &json!("static"), &Value::Null));
    assert_eq!(call.files[0].field, "thumbnail");
    client.set_custom_emoji_sticker_set_thumbnail(SET, Some(777)).await.unwrap();
    let call = stub.call("setCustomEmojiStickerSetThumbnail").await;
    assert_eq!(call.body, json!({ "name": SET, "custom_emoji_id": "777" }));
    client.delete_sticker_set(SET).await.unwrap();
    assert_eq!(stub.call("deleteStickerSet").await.body, json!({ "name": SET }));
}

#[tokio::test]
async fn log_out() {
    let mut stub = Stub::start().await;
    let client = client(&stub.url).await;
    client.log_out().await.unwrap();
    assert_eq!(stub.call("logOut").await.body, json!({}));
}
