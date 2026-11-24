//! Files for a client in Bot API mode against a local HTTP stub: media sends by file id (JSON) and from bytes
//! (multipart), albums, media edits, copies, replied messages updates carry, and downloads through `getFile`.
#![cfg(feature = "bot-api")]

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use vaegram::bot_api::Unsupported;
use vaegram::types::{FileSource, InputMedia, Media, Upload};
use vaegram::{Client, Error, filters};

const TOKEN: &str = "123:ABC";
const BOT: i64 = 99;
const USER: i64 = 7;
const GROUP: i64 = -42;
/// File ids as the Bot API gives them out.
const PHOTO: &str = "AgACAgIAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAIG9DEbXX3wSq3oI7t_PqQGAQADAgADbQADNgQ";
const DOCUMENT: &str = "BQACAgQAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAL6C87kooIPtf__________HgQ";
/// Where `getFile` puts the document.
const FILE_PATH: &str = "documents/file_1.txt";

/// A file part of a multipart request: its field, file name and bytes.
#[derive(Debug, Clone, PartialEq)]
struct Part {
    field: String,
    filename: String,
    data: Vec<u8>,
}

/// One request the stub got: the Bot API method (or `file` for a download), its fields, and its file parts. A
/// multipart request's fields are strings, as they went.
#[derive(Debug)]
struct Call {
    method: String,
    body: Value,
    files: Vec<Part>,
}

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

    /// The next request a friendly method made, past `getMe`, `getChat` and `getUpdates`.
    async fn next(&mut self) -> Call {
        loop {
            let call = tokio::time::timeout(Duration::from_secs(10), self.calls.recv())
                .await
                .expect("no such request in time")
                .expect("the stub stopped");
            if !matches!(call.method.as_str(), "getMe" | "getChat" | "getUpdates") {
                return call;
            }
        }
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
    if let Some(file) = path.strip_prefix(&format!("/file/bot{TOKEN}/")) {
        let _ = calls.send(Call { method: "file".into(), body: json!({ "path": file }), files: Vec::new() });
        let bytes = file_bytes();
        let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", bytes.len());
        let _ = stream.write_all(head.as_bytes()).await;
        let _ = stream.write_all(&bytes).await;
        let _ = stream.shutdown().await;
        return;
    }
    let prefix = format!("/bot{TOKEN}/");
    assert!(path.starts_with(&prefix), "request to {path}");
    let method = path[prefix.len()..].to_owned();
    let raw = &data[head_end..head_end + length];
    let (body, files) = match content_type.split_once("boundary=") {
        Some((_, boundary)) => multipart(raw, boundary),
        None if length == 0 => (json!({}), Vec::new()),
        None => (serde_json::from_slice(raw).unwrap(), Vec::new()),
    };
    let _ = calls.send(Call { method: method.clone(), body: body.clone(), files: files.clone() });
    let reply = match answer(&method, &body, &files, &pending).await {
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

/// The text fields of a `multipart/form-data` body as a JSON object of strings, and its file parts.
fn multipart(body: &[u8], boundary: &str) -> (Value, Vec<Part>) {
    let delimiter = format!("--{}", boundary.trim_matches('"'));
    let mut fields = serde_json::Map::new();
    let mut files = Vec::new();
    for part in split(body, delimiter.as_bytes()).into_iter().skip(1) {
        // The last delimiter ends with `--`.
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

async fn answer(method: &str, body: &Value, files: &[Part], pending: &Pending) -> Result<Value, &'static str> {
    let message_id = number(&body["message_id"]);
    let photo = json!({
        "photo": [{ "file_id": PHOTO, "file_unique_id": "p", "width": 1280, "height": 720, "file_size": 99 }]
    });
    let document = |name: Option<&str>| {
        json!({
            "document": {
                "file_id": DOCUMENT, "file_unique_id": "d", "file_name": name, "mime_type": "text/plain", "file_size": 10
            }
        })
    };
    let named = |field: &str| files.iter().find(|f| f.field == field).map(|f| f.filename.as_str());
    Ok(match method {
        "getMe" => json!({ "id": BOT, "is_bot": true, "first_name": "Bot", "username": "stub_bot" }),
        "getChat" => chat(number(&body["chat_id"])),
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
        "sendPhoto" => message(50, body, photo),
        "sendDocument" => message(52, body, document(named("document"))),
        "sendMediaGroup" => {
            let items: Vec<Value> = serde_json::from_str(body["media"].as_str().expect("media as a field")).unwrap();
            let name = files.first().map(|f| f.filename.as_str());
            json!([message(71, &items[1], document(name)), message(70, &items[0], document(None))])
        }
        "editMessageMedia" => message(message_id, &body["media"], photo),
        "copyMessage" => json!({ "message_id": 61 }),
        "getFile" => match body["file_id"].as_str() {
            // Photo ids start with `Ag`: the stub's photo is the big file.
            Some(id) if id.starts_with("Ag") => return Err("Bad Request: file is too big"),
            Some(id) => json!({ "file_id": id, "file_unique_id": "f", "file_size": FILE_SIZE, "file_path": FILE_PATH }),
            None => return Err("Bad Request: no file_id"),
        },
        _ => json!(true),
    })
}

/// A number from JSON, or from a multipart field's text.
fn number(value: &Value) -> i64 {
    value.as_i64().or_else(|| value.as_str()?.parse().ok()).unwrap_or(0)
}

fn chat(id: i64) -> Value {
    if id > 0 {
        json!({ "id": id, "type": "private", "first_name": "Ann" })
    } else {
        json!({ "id": id, "type": "group", "title": "Group" })
    }
}

/// The bot's message after a send or an edit, in the chat `fields` name (else the user's), with the caption
/// `fields` carry and `media` merged in.
fn message(id: i64, fields: &Value, media: Value) -> Value {
    let chat_id = match number(&fields["chat_id"]) {
        0 => USER,
        id => id,
    };
    let mut message = json!({
        "message_id": id,
        "date": unix_now(),
        "chat": chat(chat_id),
        "from": { "id": BOT, "is_bot": true, "first_name": "Bot" },
    });
    if let Some(caption) = fields["caption"].as_str() {
        message["caption"] = caption.into();
    }
    if let (Value::Object(object), Value::Object(media)) = (&mut message, media) {
        object.extend(media);
    }
    message
}

/// The document the file server serves: over two 512 KiB chunks, so a download comes in several.
const FILE_SIZE: usize = 1_200_000;

fn file_bytes() -> Vec<u8> {
    (0..FILE_SIZE).map(|i| (i % 251) as u8).collect()
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

async fn client(url: &str) -> Client {
    Client::builder("bot_api_media", 1, "hash").bot_api(TOKEN).bot_api_url(url).catch_up(true).start().await.unwrap()
}

#[tokio::test]
async fn a_photo_by_file_id_goes_as_json() {
    let mut stub = Stub::start(Vec::new()).await;
    let client = client(&stub.url).await;

    let sent = client.send_photo(USER, FileSource::file_id(PHOTO).unwrap()).caption("look").await.unwrap();
    let call = stub.next().await;
    assert_eq!(call.method, "sendPhoto");
    assert!(call.files.is_empty());
    // The file id goes as the Bot API gave it out, and the JSON keeps numbers as numbers.
    assert_eq!((&call.body["chat_id"], &call.body["photo"]), (&json!(USER), &json!(PHOTO)));
    assert_eq!(call.body["caption"], json!("look"));

    assert_eq!((sent.id, sent.chat.id, sent.caption()), (50, USER, Some("look")));
    let photo = sent.media.as_ref().and_then(Media::photo).expect("a photo");
    assert_eq!((photo.width, photo.height, photo.file_size), (1280, 720, 99));
}

#[tokio::test]
async fn a_document_from_bytes_goes_as_one_multipart_request() {
    let mut stub = Stub::start(Vec::new()).await;
    let client = client(&stub.url).await;

    let document = Upload::bytes("notes.txt", b"hello file");
    let thumb = Upload::bytes("thumb.jpg", b"jpeg");
    let sent = client.send_document(USER, document).thumb(thumb).caption("doc").await.unwrap();
    let call = stub.next().await;
    assert_eq!(call.method, "sendDocument");
    let part = |field: &str, filename: &str, data: &[u8]| Part {
        field: field.into(),
        filename: filename.into(),
        data: data.to_vec(),
    };
    assert_eq!(call.files, [part("document", "notes.txt", b"hello file"), part("thumbnail", "thumb.jpg", b"jpeg")]);
    assert_eq!((&call.body["chat_id"], &call.body["caption"]), (&json!("7"), &json!("doc")));
    assert!(call.body.get("document").is_none(), "the upload went as a part only");

    assert_eq!((sent.id, sent.caption()), (52, Some("doc")));
    let document = sent.media.as_ref().and_then(Media::document).expect("a document");
    assert_eq!((document.file_name.as_deref(), document.file_size), (Some("notes.txt"), 10));
}

#[tokio::test]
async fn an_album_of_two_goes_as_one_send_media_group() {
    let mut stub = Stub::start(Vec::new()).await;
    let client = client(&stub.url).await;

    let stored = InputMedia::document(FileSource::file_id(DOCUMENT).unwrap()).caption("one");
    let uploaded = InputMedia::document(Upload::bytes("b.txt", b"second"));
    let sent = client.send_media_group(USER, [stored, uploaded]).await.unwrap();
    let call = stub.next().await;
    assert_eq!(call.method, "sendMediaGroup");
    let media: Value = serde_json::from_str(call.body["media"].as_str().unwrap()).unwrap();
    assert_eq!(
        media,
        json!([
            { "type": "document", "media": DOCUMENT, "caption": "one" },
            { "type": "document", "media": "attach://file0" },
        ])
    );
    let part = Part { field: "file0".into(), filename: "b.txt".into(), data: b"second".to_vec() };
    assert_eq!(call.files, [part]);

    // Sorted by id, whatever order the answer had.
    assert_eq!(sent.iter().map(|m| m.id).collect::<Vec<_>>(), [70, 71]);
    assert_eq!(sent[0].caption(), Some("one"));
}

#[tokio::test]
async fn edit_message_media_and_copy_message_go_straight_to_the_bot_api() {
    let mut stub = Stub::start(Vec::new()).await;
    let client = client(&stub.url).await;

    let photo = InputMedia::photo(FileSource::file_id(PHOTO).unwrap()).caption("edited").has_spoiler();
    let edited = client.edit_message_media(USER, 50, photo).await.unwrap();
    let call = stub.next().await;
    assert_eq!(call.method, "editMessageMedia");
    assert_eq!((&call.body["chat_id"], &call.body["message_id"]), (&json!(USER), &json!(50)));
    let media = json!({ "type": "photo", "media": PHOTO, "caption": "edited", "has_spoiler": true });
    assert_eq!(call.body["media"], media);
    assert_eq!((edited.id, edited.caption()), (50, Some("edited")));
    assert!(edited.media.as_ref().and_then(Media::photo).is_some());

    // Nothing reads the source first: the Bot API has no way to.
    let copy = client.copy_message(USER, GROUP, 50).caption("new caption").await.unwrap();
    let call = stub.next().await;
    assert_eq!(call.method, "copyMessage");
    let fields = (&call.body["chat_id"], &call.body["from_chat_id"], &call.body["message_id"]);
    assert_eq!(fields, (&json!(USER), &json!(GROUP), &json!(50)));
    assert_eq!(call.body["caption"], json!("new caption"));
    assert_eq!((copy.id, copy.chat.id, copy.outgoing), (61, USER, true));
}

#[tokio::test]
async fn reply_to_message_takes_the_replied_message_the_update_carried() {
    let ann = json!({ "id": USER, "type": "private", "first_name": "Ann" });
    let from = json!({ "id": USER, "is_bot": false, "first_name": "Ann" });
    let carried = json!({
        "update_id": 1,
        "message": {
            "message_id": 10, "date": unix_now(), "chat": ann, "from": from, "text": "a reply",
            "reply_to_message": {
                "message_id": 9, "date": unix_now(), "chat": ann,
                "from": { "id": BOT, "is_bot": true, "first_name": "Bot" }, "text": "original"
            }
        }
    });
    // A reply the Bot API sent without the message it replies to.
    let bare = json!({
        "update_id": 2,
        "message": {
            "message_id": 12, "date": unix_now(), "chat": ann, "from": from, "text": "another",
            "external_reply": { "origin": { "type": "hidden_user", "date": 1, "sender_user_name": "Bo" }, "message_id": 4 }
        }
    });
    let stub = Stub::start(vec![carried, bare]).await;
    let client = client(&stub.url).await;
    let (tx, mut results) = mpsc::unbounded_channel();
    client.on_message(filters::all(), move |m| {
        let tx = tx.clone();
        async move {
            let replied = m.reply_to_message().await.map(|r| r.map(|r| (r.id, r.text().to_owned())));
            let _ = tx.send((m.id, replied.map_err(|e| e.downcast_ref::<Unsupported>().map(Unsupported::what))));
            Ok::<_, Error>(())
        }
    });
    let runner = client.clone();
    let run = tokio::spawn(async move { runner.run().await });

    let mut seen = Vec::new();
    for _ in 0..2 {
        seen.push(tokio::time::timeout(Duration::from_secs(10), results.recv()).await.unwrap().unwrap());
    }
    seen.sort_by_key(|(id, _)| *id);
    assert_eq!(seen[0], (10, Ok(Some((9, "original".to_owned())))));
    assert_eq!(seen[1], (12, Err(Some("a replied message its update didn't carry"))));

    client.stop();
    tokio::time::timeout(Duration::from_secs(10), run).await.expect("run didn't stop").unwrap().unwrap();
}

#[tokio::test]
async fn a_download_goes_through_get_file_and_the_file_url() {
    let mut stub = Stub::start(Vec::new()).await;
    let client = client(&stub.url).await;

    let bytes = client.download_media(DOCUMENT).await.unwrap();
    assert!(bytes == file_bytes(), "the file came back whole");
    let call = stub.next().await;
    assert_eq!(call.method, "getFile");
    assert!(call.body["file_id"].as_str().is_some_and(|id| id.starts_with("BQ")), "a document id");
    let call = stub.next().await;
    assert_eq!((call.method.as_str(), &call.body["path"]), ("file", &json!(FILE_PATH)));

    // A stream gets the same bytes in order, and a range only its part.
    let mut stream = client.download_media(DOCUMENT).stream();
    let mut streamed = Vec::new();
    while let Some(chunk) = stream.next().await.unwrap() {
        streamed.extend_from_slice(&chunk);
    }
    assert!(streamed == file_bytes(), "the stream came back whole");
    let range = client.download_media(DOCUMENT).offset(600_000).limit(10).await.unwrap();
    assert_eq!(range, file_bytes()[600_000..600_010]);
}

#[tokio::test]
async fn a_file_over_20_mb_is_refused_before_it_downloads() {
    let mut stub = Stub::start(Vec::new()).await;
    let client = client(&stub.url).await;

    let error = client.download_media(PHOTO).await.unwrap_err();
    assert!(matches!(error, Error::InvalidArgument(m) if m.contains("20 MB")), "{error:?}");
    assert_eq!(stub.next().await.method, "getFile");
    while let Ok(call) = stub.calls.try_recv() {
        assert_ne!(call.method, "file", "nothing is downloaded");
    }
}
