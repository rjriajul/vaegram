use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use serde_json::Value;

use super::Request;
use super::input_file::{self, Attachment};
use crate::errors::{Error, Result, RpcError};

const DEFAULT_URL: &str = "https://api.telegram.org";
// Long enough for a getUpdates long poll of up to 50 seconds plus the answer.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(75);
// The most a connection may take to open, whatever the request timeout.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// A bot that talks to Telegram through the Bot API over HTTPS, instead of an MTProto session.
///
/// Cloning is cheap and every clone shares one connection pool.
#[derive(Clone)]
pub struct BotApi(Arc<Inner>);

struct Inner {
    http: reqwest::Client,
    // `<url>/bot<token>`: the token is in every request URL, so URLs are never printed.
    base: String,
    files: String,
    // The whole of a JSON request, or the longest wait for a byte of a file.
    timeout: Duration,
}

/// A group that became a supergroup: the Bot API refused a request to the group and named the supergroup, which
/// takes the group's requests from now on. It comes inside `Error::Other`; [`Error::downcast_ref`] reads it back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChatMigrated(i64);

impl ChatMigrated {
    /// The supergroup's id, Bot API style (`-100…`).
    pub fn chat_id(&self) -> i64 {
        self.0
    }
}

impl fmt::Display for ChatMigrated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the group is now the supergroup {}; send to that chat instead", self.0)
    }
}

impl std::error::Error for ChatMigrated {}

/// Builder for [`BotApi`], made by [`BotApi::builder`].
#[must_use]
pub struct BotApiBuilder {
    token: String,
    url: String,
    timeout: Duration,
}

impl BotApi {
    /// A bot with this token from @BotFather, on `api.telegram.org`.
    pub fn new(token: impl Into<String>) -> Result<BotApi> {
        BotApi::builder(token).build()
    }

    /// A builder to set the server URL or the request timeout before connecting.
    pub fn builder(token: impl Into<String>) -> BotApiBuilder {
        BotApiBuilder { token: token.into(), url: DEFAULT_URL.into(), timeout: DEFAULT_TIMEOUT }
    }

    /// Sends one Bot API method and returns its `result`.
    ///
    /// A request holding an [`InputFile::Upload`](super::InputFile::Upload) goes as `multipart/form-data`,
    /// others as JSON. Telegram's errors come back as [`Error::Rpc`]; a `retry_after` becomes `420 FLOOD_WAIT_<n>`,
    /// so [`Error::flood_wait`] works as for MTProto, and a `migrate_to_chat_id` becomes a [`ChatMigrated`] in
    /// `Error::Other`. A JSON request may take the builder's [`timeout`](BotApiBuilder::timeout) in all; an upload
    /// isn't bounded as a whole, as a big file on a slow link takes as long as it takes.
    pub async fn invoke<R: Request>(&self, request: &R) -> Result<R::Response> {
        let (json, uploads) = input_file::collect(request).map_err(invalid)?;
        self.call_with_files(R::NAME, json, uploads).await
    }

    /// Sends `method` with a JSON body built by hand and returns its `result` as it came, for the bridge, which
    /// reads only the fields it needs.
    pub(crate) async fn call(&self, method: &str, body: &Value) -> Result<Value> {
        let url = format!("{}/{method}", self.0.base);
        let body = serde_json::to_vec(body).map_err(invalid)?;
        let request = self.0.http.post(url).header(reqwest::header::CONTENT_TYPE, "application/json").body(body);
        read(request.timeout(self.0.timeout)).await
    }

    /// [`call`](Self::call) with files: with any upload the body goes as `multipart/form-data`, every upload as a
    /// part, else as JSON. `body` names each upload by an [`input_file::marker`].
    pub(crate) async fn call_with_files<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        body: Value,
        uploads: Vec<Attachment>,
    ) -> Result<T> {
        if uploads.is_empty() {
            let value = self.call(method, &body).await?;
            return serde_json::from_value(value).map_err(invalid);
        }
        let url = format!("{}/{method}", self.0.base);
        read(self.0.http.post(url).multipart(form(body, uploads))).await
    }

    /// Starts downloading the file `getFile` named by `file_path`; its bytes are read as they arrive. No byte may
    /// take longer than the builder's timeout to come, the answer's head included, but the whole file may.
    pub(crate) async fn fetch(&self, file_path: &str) -> Result<FileBody> {
        let request = self.0.http.get(self.file_url(file_path)).send();
        let response = tokio::time::timeout(self.0.timeout, request).await.map_err(|_| Error::Timeout)?;
        let response = response.map_err(network)?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::Io(std::io::Error::other(format!("the Bot API file server answered {status}"))));
        }
        Ok(FileBody { response, timeout: self.0.timeout })
    }

    /// The URL to download a file from, given the `file_path` that `getFile` returned. It holds the bot's token, so
    /// it must not be shown to anyone.
    pub fn file_url(&self, file_path: &str) -> String {
        format!("{}/{file_path}", self.0.files)
    }
}

impl BotApiBuilder {
    /// The Bot API server, `https://api.telegram.org` unless set; a local Bot API server's URL goes here.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    /// How long one JSON request may take, 75 seconds unless set; it must be longer than any `getUpdates` timeout.
    /// A file download may take as long as it needs, but waits this long at most for each piece of it.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Builds the client. Nothing is sent yet.
    pub fn build(self) -> Result<BotApi> {
        let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .map_err(|e| Error::Io(std::io::Error::other(e)))?
            .with_root_certificates(roots)
            .with_no_client_auth();
        // No timeout for whole requests here: JSON requests set their own, and file transfers have none.
        let connect = self.timeout.min(CONNECT_TIMEOUT);
        let http = reqwest::Client::builder().tls_backend_preconfigured(tls).connect_timeout(connect);
        let http = http.build().map_err(network)?;
        let url = self.url.trim_end_matches('/');
        let base = format!("{url}/bot{}", self.token);
        let files = format!("{url}/file/bot{}", self.token);
        Ok(BotApi(Arc::new(Inner { http, base, files, timeout: self.timeout })))
    }
}

/// A file being downloaded from the Bot API's file server.
pub(crate) struct FileBody {
    response: reqwest::Response,
    // The longest wait for the next piece.
    timeout: Duration,
}

impl FileBody {
    /// Appends the next piece of the file to `buf`; `false`, with nothing appended, once the file has ended.
    pub(crate) async fn read_into(&mut self, buf: &mut Vec<u8>) -> Result<bool> {
        let piece = tokio::time::timeout(self.timeout, self.response.chunk()).await.map_err(|_| Error::Timeout)?;
        match piece.map_err(network)? {
            Some(piece) => {
                buf.extend_from_slice(&piece);
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

/// The multipart body: an upload's bytes are moved into its part, not copied.
fn form(json: Value, uploads: Vec<Attachment>) -> Form {
    form_fields(json, uploads).into_iter().fold(Form::new(), |form, field| match field {
        Field::Text(key, text) => form.text(key, text),
        Field::File(key, name, data) => form.part(key, Part::bytes(data).file_name(name)),
    })
}

#[derive(Debug, PartialEq)]
enum Field {
    Text(String, String),
    File(String, String, Vec<u8>),
}

/// The fields of a multipart body: a top-level field that is an upload's [`input_file::marker`] carries the upload
/// itself, a marker deeper in (inside `media` JSON) becomes `attach://<key>` with the upload a part under that key,
/// and every other field is text, whatever it says.
fn form_fields(json: Value, uploads: Vec<Attachment>) -> Vec<Field> {
    let mut uploads: Vec<Option<Attachment>> = uploads.into_iter().map(Some).collect();
    let mut fields = Vec::new();
    let Value::Object(object) = json else { return fields };
    for (key, mut value) in object {
        let marked = input_file::marked(&value);
        let own = marked.and_then(|k| uploads.iter_mut().find(|u| u.as_ref().is_some_and(|u| u.0 == k)));
        if let Some((_, name, data)) = own.and_then(Option::take) {
            fields.push(Field::File(key, name, data));
            continue;
        }
        input_file::attach(&mut value);
        let text = match value {
            Value::Null => continue,
            Value::String(s) => s,
            other => other.to_string(),
        };
        fields.push(Field::Text(key, text));
    }
    fields.extend(uploads.into_iter().flatten().map(|(key, name, data)| Field::File(key, name, data)));
    fields
}

#[derive(Deserialize)]
struct Envelope {
    ok: bool,
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error_code: Option<i32>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    parameters: Option<Parameters>,
}

#[derive(Deserialize)]
struct Parameters {
    #[serde(default)]
    retry_after: Option<u32>,
    #[serde(default)]
    migrate_to_chat_id: Option<i64>,
}

impl Envelope {
    fn into_result<T: serde::de::DeserializeOwned>(self) -> Result<T> {
        if self.ok {
            return serde_json::from_value(self.result.unwrap_or(Value::Null)).map_err(invalid);
        }
        let code = self.error_code.unwrap_or(0);
        match self.parameters {
            Some(Parameters { retry_after: Some(seconds), .. }) => {
                return Err(Error::Rpc(RpcError::new(420, "FLOOD_WAIT", Some(seconds))));
            }
            Some(Parameters { migrate_to_chat_id: Some(chat_id), .. }) => {
                return Err(Error::other(ChatMigrated(chat_id)));
            }
            _ => {}
        }
        Err(Error::Rpc(RpcError::new(code, error_name(self.description.as_deref().unwrap_or("")), None)))
    }
}

/// `Bad Request: chat not found` -> `CHAT_NOT_FOUND`, so names read like MTProto's.
// shortcut: the Bot API's descriptions aren't a stable list; map them to MTProto names when a caller needs to match.
fn error_name(description: &str) -> String {
    let text = description.split_once(": ").map_or(description, |(_, rest)| rest);
    let mut name: String =
        text.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' }).collect();
    while name.contains("__") {
        name = name.replace("__", "_");
    }
    name.trim_matches('_').to_owned()
}

/// Sends `request` and reads the envelope Telegram answers with.
async fn read<T: serde::de::DeserializeOwned>(request: reqwest::RequestBuilder) -> Result<T> {
    let response = request.send().await.map_err(network)?;
    let body = response.bytes().await.map_err(network)?;
    let envelope: Envelope = serde_json::from_slice(&body).map_err(invalid)?;
    envelope.into_result()
}

pub(super) fn invalid(e: serde_json::Error) -> Error {
    Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

// `without_url` keeps the token, which is in every request URL, out of the error.
fn network(e: reqwest::Error) -> Error {
    if e.is_timeout() { Error::Timeout } else { Error::Io(std::io::Error::other(e.without_url())) }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::{BotApi, ChatMigrated, Envelope, Field, error_name, form_fields};
    use crate::bot_api::input_file::marker;
    use crate::errors::Error;

    fn envelope(json: &str) -> Envelope {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn answers_become_results_or_rpc_errors() {
        let ok: i64 = envelope(r#"{"ok":true,"result":42}"#).into_result().unwrap();
        assert_eq!(ok, 42);

        let flood = envelope(
            r#"{"ok":false,"error_code":429,"description":"Too Many Requests: retry after 7","parameters":{"retry_after":7}}"#,
        );
        let Err(Error::Rpc(e)) = flood.into_result::<bool>() else { panic!("expected an rpc error") };
        assert_eq!(e.flood_wait(), Some(std::time::Duration::from_secs(7)));

        let missing = envelope(r#"{"ok":false,"error_code":400,"description":"Bad Request: chat not found"}"#);
        let Err(Error::Rpc(e)) = missing.into_result::<bool>() else { panic!("expected an rpc error") };
        assert_eq!((e.code, e.name.as_str()), (400, "CHAT_NOT_FOUND"));
    }

    #[test]
    fn a_migrated_group_names_its_supergroup() {
        let moved = envelope(
            r#"{"ok":false,"error_code":400,"description":"Bad Request: group chat was upgraded to a supergroup chat","parameters":{"migrate_to_chat_id":-1001}}"#,
        );
        let error = moved.into_result::<bool>().unwrap_err();
        assert_eq!(error.downcast_ref::<ChatMigrated>().map(ChatMigrated::chat_id), Some(-1001));
        assert!(error.to_string().contains("-1001"), "{error}");
    }

    #[test]
    fn error_names_read_like_mtproto() {
        assert_eq!(error_name("Forbidden: bot was blocked by the user"), "BOT_WAS_BLOCKED_BY_THE_USER");
        assert_eq!(error_name("Bad Request: PEER_ID_INVALID"), "PEER_ID_INVALID");
        assert_eq!(error_name("Unauthorized"), "UNAUTHORIZED");
    }

    #[test]
    fn top_level_uploads_are_parts_and_nested_ones_go_by_key() {
        let file = |key: &str, name: &str| (key.to_owned(), name.to_owned(), b"x".to_vec());
        let uploads = vec![file("file0", "a.txt"), file("file1", "b.jpg")];
        let json = json!({
            "chat_id": 5,
            "document": marker("file0"),
            "media": [{ "type": "photo", "media": marker("file1") }],
            "thumbnail": null,
        });
        // serde_json keeps keys sorted or in insertion order depending on its features; both give this order.
        let media = json!([{ "type": "photo", "media": "attach://file1" }]).to_string();
        assert_eq!(
            form_fields(json, uploads),
            [
                Field::Text("chat_id".into(), "5".into()),
                Field::File("document".into(), "a.txt".into(), b"x".to_vec()),
                Field::Text("media".into(), media),
                Field::File("file1".into(), "b.jpg".into(), b"x".to_vec()),
            ]
        );
    }

    #[test]
    fn text_that_reads_like_an_attachment_stays_text() {
        let uploads = vec![("file0".to_owned(), "a.jpg".to_owned(), b"x".to_vec())];
        let json = json!({ "caption": "attach://file0", "photo": marker("file0") });
        assert_eq!(
            form_fields(json, uploads),
            [
                Field::Text("caption".into(), "attach://file0".into()),
                Field::File("photo".into(), "a.jpg".into(), b"x".to_vec()),
            ]
        );
    }

    /// A server that takes one request and answers with `pieces` of a body, each after `gap`, the head at once.
    async fn slow_server(pieces: Vec<&'static [u8]>, gap: Duration) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let _ = stream.read(&mut [0u8; 4096]).await;
            let length: usize = pieces.iter().map(|p| p.len()).sum();
            let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n");
            let _ = stream.write_all(head.as_bytes()).await;
            for piece in pieces {
                tokio::time::sleep(gap).await;
                let _ = stream.write_all(piece).await;
            }
        });
        url
    }

    fn api(url: &str, timeout: Duration) -> BotApi {
        BotApi::builder("1:x").url(url).timeout(timeout).build().unwrap()
    }

    #[tokio::test]
    async fn a_json_request_is_bounded_as_a_whole() {
        let url = slow_server(vec![&br#"{"ok":true,"result":true}"#[..]], Duration::from_millis(500)).await;
        let error = api(&url, Duration::from_millis(150)).call("getMe", &json!({})).await.unwrap_err();
        assert!(matches!(error, Error::Timeout), "{error:?}");
    }

    #[tokio::test]
    async fn a_file_may_take_longer_than_the_timeout_but_not_stall() {
        let pieces = vec![&b"abcde"[..]; 5];
        let url = slow_server(pieces, Duration::from_millis(100)).await;
        let mut file = api(&url, Duration::from_millis(300)).fetch("f").await.unwrap();
        let mut got = Vec::new();
        while file.read_into(&mut got).await.unwrap() {}
        // Half a second in all, past the 300 ms a JSON request may take.
        assert_eq!(got, b"abcde".repeat(5));

        let url = slow_server(vec![&b"ab"[..], &b"cd"[..]], Duration::from_millis(500)).await;
        let mut file = api(&url, Duration::from_millis(150)).fetch("f").await.unwrap();
        let error = file.read_into(&mut Vec::new()).await.unwrap_err();
        assert!(matches!(error, Error::Timeout), "{error:?}");
    }
}
