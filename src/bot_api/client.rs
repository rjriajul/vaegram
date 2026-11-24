use std::sync::Arc;
use std::time::Duration;

use reqwest::multipart::{Form, Part};
use serde::Deserialize;

use super::Request;
use super::input_file::{self, Attachment};
use crate::errors::{Error, Result, RpcError};

const DEFAULT_URL: &str = "https://api.telegram.org";
// Long enough for a getUpdates long poll of up to 50 seconds plus the answer.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(75);

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
}

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
    /// so [`Error::flood_wait`] works as for MTProto.
    pub async fn invoke<R: Request>(&self, request: &R) -> Result<R::Response> {
        let (json, uploads) = input_file::collect(request).map_err(invalid)?;
        let url = format!("{}/{}", self.0.base, R::NAME);
        let builder = self.0.http.post(url);
        let builder = if uploads.is_empty() {
            let body = serde_json::to_vec(&json).map_err(invalid)?;
            builder.header(reqwest::header::CONTENT_TYPE, "application/json").body(body)
        } else {
            builder.multipart(form(json, uploads))
        };
        read(builder).await
    }

    /// Sends `method` with a JSON body built by hand and returns its `result` as it came, for the bridge, which
    /// reads only the fields it needs.
    pub(crate) async fn call(&self, method: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        let url = format!("{}/{method}", self.0.base);
        let body = serde_json::to_vec(body).map_err(invalid)?;
        read(self.0.http.post(url).header(reqwest::header::CONTENT_TYPE, "application/json").body(body)).await
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

    /// How long one request may take, 75 seconds unless set. It must be longer than any `getUpdates` timeout.
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
        let http =
            reqwest::Client::builder().tls_backend_preconfigured(tls).timeout(self.timeout).build().map_err(network)?;
        let url = self.url.trim_end_matches('/');
        let base = format!("{url}/bot{}", self.token);
        let files = format!("{url}/file/bot{}", self.token);
        Ok(BotApi(Arc::new(Inner { http, base, files })))
    }
}

/// The multipart body: a top-level file field carries the upload itself, a nested `attach://<key>` (inside
/// `media` JSON) names a part sent under that key, and every other field is text.
fn form(json: serde_json::Value, uploads: Vec<Attachment>) -> Form {
    form_fields(json, uploads).into_iter().fold(Form::new(), |form, field| match field {
        Field::Text(key, text) => form.text(key, text),
        Field::File(key, name, data) => form.part(key, Part::bytes(data.to_vec()).file_name(name)),
    })
}

#[derive(Debug, PartialEq)]
enum Field {
    Text(String, String),
    File(String, String, std::sync::Arc<[u8]>),
}

fn form_fields(json: serde_json::Value, uploads: Vec<Attachment>) -> Vec<Field> {
    let mut uploads: Vec<Option<Attachment>> = uploads.into_iter().map(Some).collect();
    let mut fields = Vec::new();
    let serde_json::Value::Object(object) = json else { return fields };
    for (key, value) in object {
        let text = match value {
            serde_json::Value::Null => continue,
            serde_json::Value::String(s) => s,
            other => other.to_string(),
        };
        let attach = text.strip_prefix("attach://");
        let own = attach.and_then(|k| uploads.iter_mut().find(|u| u.as_ref().is_some_and(|u| u.0 == k)));
        match own.and_then(Option::take) {
            Some((_, name, data)) => fields.push(Field::File(key, name, data)),
            None => fields.push(Field::Text(key, text)),
        }
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
}

impl Envelope {
    fn into_result<T: serde::de::DeserializeOwned>(self) -> Result<T> {
        if self.ok {
            return serde_json::from_value(self.result.unwrap_or(serde_json::Value::Null)).map_err(invalid);
        }
        let code = self.error_code.unwrap_or(0);
        if let Some(seconds) = self.parameters.and_then(|p| p.retry_after) {
            return Err(Error::Rpc(RpcError::new(420, "FLOOD_WAIT", Some(seconds))));
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
    use super::{Envelope, Field, error_name, form_fields};
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
    fn error_names_read_like_mtproto() {
        assert_eq!(error_name("Forbidden: bot was blocked by the user"), "BOT_WAS_BLOCKED_BY_THE_USER");
        assert_eq!(error_name("Bad Request: PEER_ID_INVALID"), "PEER_ID_INVALID");
        assert_eq!(error_name("Unauthorized"), "UNAUTHORIZED");
    }

    #[test]
    fn top_level_uploads_are_parts_and_nested_ones_go_by_key() {
        let data: std::sync::Arc<[u8]> = b"x".to_vec().into();
        let file = |key: &str, name: &str| (key.to_owned(), name.to_owned(), data.clone());
        let uploads = vec![file("file0", "a.txt"), file("file1", "b.jpg")];
        let media = serde_json::json!([{"type": "photo", "media": "attach://file1"}]);
        let json = serde_json::json!({
            "chat_id": 5,
            "document": "attach://file0",
            "media": media.clone(),
            "thumbnail": null,
        });
        // serde_json keeps keys sorted or in insertion order depending on its features; match whichever it uses.
        let media = media.to_string();
        assert_eq!(
            form_fields(json, uploads),
            [
                Field::Text("chat_id".into(), "5".into()),
                Field::File("document".into(), "a.txt".into(), data.clone()),
                Field::Text("media".into(), media),
                Field::File("file1".into(), "b.jpg".into(), data),
            ]
        );
    }
}
