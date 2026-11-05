//! The web-proxy carrier: one MTProxy stream carried in HTTPS requests to a relay, for networks that only let HTTPS
//! out. `POST /api/v1/session` with the bridge capability opens a session; frames go up in `POST …/up?seq=` bodies
//! and come down in `GET …/down?cursor=&wait=` long polls. Each direction starts with a 4 MiB window that the other
//! side tops up with WINDOW frames.

use std::collections::VecDeque;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use super::{ConnectionFactory, Secret, Stream, dial_any, http1, tls};
use crate::crypto::hmac_sha256;
use crate::errors::{Error, Result};
use crate::utils::base64url_encode;

const BRIDGE_CONTEXT: &[u8] = b"tdesktop-web-proxy-bridge-v1\n";
const SESSION: &str = "/api/v1/session";

const OPEN: u8 = 0x01;
const DATA: u8 = 0x02;
const CLOSE: u8 = 0x03;
const WINDOW: u8 = 0x04;
const PING: u8 = 0x05;
const PONG: u8 = 0x06;
const HELLO: u8 = 0x10;
const WELCOME: u8 = 0x11;
const AUTH_CHALLENGE: u8 = 0x12;
const AUTH_RESPONSE: u8 = 0x13;
const BYE: u8 = 0x1f;
const KINDS: [u8; 11] = [OPEN, DATA, CLOSE, WINDOW, PING, PONG, HELLO, WELCOME, AUTH_CHALLENGE, AUTH_RESPONSE, BYE];

// Stream 0 carries the session's own frames; the carrier opens stream 1 and no other.
const CONTROL: u32 = 0;
const STREAM: u32 = 1;

const FRAME_HEADER: usize = 8;
const MAX_PAYLOAD: usize = 1 << 20;
// The relay batches up to 2 MiB of frames into one answer.
const MAX_RESPONSE: usize = 4 << 20;
const INITIAL_WINDOW: u32 = 4 << 20;
const UPLINK_FRAME: usize = 64 << 10;
// Downlink credit goes back once this much has been handed on, or GRANT_DELAY after the first byte of it.
const GRANT_THRESHOLD: u32 = 256 << 10;
const GRANT_DELAY: Duration = Duration::from_millis(20);
// What the relay's data may get ahead of the reader by before the carrier stops taking more.
const PIPE: usize = 256 << 10;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const LONG_POLL_WAIT: Duration = Duration::from_secs(25);
const LONG_POLL_MARGIN: Duration = Duration::from_secs(10);
const WELCOME_TIMEOUT: Duration = Duration::from_secs(30);
const CREDIT_TIMEOUT: Duration = Duration::from_secs(30);

/// Opens a carrier to the relay at `host`, through `factory` when one is set, and returns the stream it carries.
/// The MTProxy obfuscation with `secret` goes on top, as on a TCP connection to an MTProxy.
pub(crate) async fn open(
    host: &str,
    secret: &Secret,
    factory: Option<Arc<dyn ConnectionFactory>>,
) -> Result<DuplexStream> {
    let host = canonical_host(host)?;
    if secret.domain.is_some() {
        return Err(Error::InvalidArgument("a web proxy takes a plain or dd secret, not an ee one"));
    }
    let capability = bridge_capability(&host, secret);
    let host: Arc<str> = host.into();
    Ok(start(host.clone(), dialer(host, factory), &capability).await?)
}

/// The form of a relay's host name the bridge capability is made from: lowercase, without a trailing dot. An IP
/// address, a one-label name and anything but ASCII letters, digits and hyphens between the dots are refused.
pub(crate) fn canonical_host(host: &str) -> Result<String> {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    let label_ok = |label: &str| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    };
    let valid = host.contains('.') && host.parse::<std::net::IpAddr>().is_err() && host.split('.').all(label_ok);
    valid.then_some(host).ok_or(Error::InvalidArgument("a web proxy's host must be a DNS name of two labels or more"))
}

/// Proof the client holds the secret for this relay, without the secret: HMAC-SHA256 keyed by the secret as shared
/// (a `dd` secret keeps its marker), over a fixed context and the host, in unpadded base64url.
fn bridge_capability(host: &str, secret: &Secret) -> String {
    let mut key = Vec::with_capacity(17);
    if secret.padded {
        key.push(0xdd);
    }
    key.extend_from_slice(&secret.key);
    base64url_encode(&hmac_sha256(&key, &[BRIDGE_CONTEXT, host.as_bytes()]))
}

#[derive(Debug, PartialEq, Eq)]
struct Frame {
    kind: u8,
    stream: u32,
    payload: Vec<u8>,
}

/// Appends a frame: kind, a 24-bit stream id and a 32-bit length, big-endian, then the payload.
fn push_frame(out: &mut Vec<u8>, kind: u8, stream: u32, payload: &[u8]) {
    out.push(kind);
    out.extend_from_slice(&stream.to_be_bytes()[1..]);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
}

fn frame(kind: u8, stream: u32, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(FRAME_HEADER + payload.len());
    push_frame(&mut out, kind, stream, payload);
    out
}

/// A response body: one or more whole frames, nothing else.
fn parse_frames(mut body: &[u8]) -> io::Result<Vec<Frame>> {
    if body.is_empty() {
        return Err(invalid("the web proxy relay sent an empty batch"));
    }
    let mut frames = Vec::new();
    while !body.is_empty() {
        let Some((header, rest)) = body.split_first_chunk::<FRAME_HEADER>() else {
            return Err(invalid("the web proxy relay sent a cut frame"));
        };
        if !KINDS.contains(&header[0]) {
            return Err(invalid("the web proxy relay sent an unknown frame"));
        }
        let stream = u32::from_be_bytes([0, header[1], header[2], header[3]]);
        let len = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize;
        if len > MAX_PAYLOAD {
            return Err(invalid("the web proxy relay sent a frame over 1 MiB"));
        }
        let (payload, rest) =
            rest.split_at_checked(len).ok_or_else(|| invalid("the web proxy relay sent a cut frame"))?;
        frames.push(Frame { kind: header[0], stream, payload: payload.to_vec() });
        body = rest;
    }
    Ok(frames)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// The session's id and the cursor its downlink starts from, out of `{"id": "…", "cursor": N}`.
fn session(answer: &[u8]) -> io::Result<(String, u64)> {
    let bad = || invalid("the web proxy relay's session answer isn't understood");
    let text = std::str::from_utf8(answer).map_err(|_| bad())?;
    let id = json_value(text, "id").and_then(|v| v.strip_prefix('"')?.split('"').next());
    // The id goes into request paths, so only characters that need no escaping are taken.
    let id = id.filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)));
    let cursor = match json_value(text, "cursor") {
        None => Some(0),
        Some(value) => value.split(|c: char| !c.is_ascii_digit()).next().and_then(|digits| digits.parse().ok()),
    };
    match (id, cursor) {
        (Some(id), Some(cursor)) => Ok((id.to_owned(), cursor)),
        _ => Err(bad()),
    }
}

/// What follows `"key":` in a flat JSON object.
fn json_value<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let at = json.find(&format!("\"{key}\""))?;
    json[at + key.len() + 2..].trim_start().strip_prefix(':').map(str::trim_start)
}

type Dial = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = io::Result<Box<dyn Stream>>> + Send>> + Send + Sync>;

/// TLS to `host`:443, over TCP or what the connection factory opens.
fn dialer(host: Arc<str>, factory: Option<Arc<dyn ConnectionFactory>>) -> Dial {
    Arc::new(move || {
        let (host, factory) = (host.clone(), factory.clone());
        Box::pin(async move {
            let stream: Box<dyn Stream> = match &factory {
                Some(factory) => dial_any(factory.as_ref(), (&*host, 443)).await?,
                None => {
                    let stream = TcpStream::connect((&*host, 443)).await?;
                    stream.set_nodelay(true)?;
                    Box::new(stream)
                }
            };
            Ok(Box::new(tls::connect(stream, &host).await?) as Box<dyn Stream>)
        })
    })
}

/// An HTTP/1.1 client on one kept-alive connection to the relay.
struct Http {
    host: Arc<str>,
    dial: Dial,
    conn: Option<Box<dyn Stream>>,
    pending: Vec<u8>,
}

impl Http {
    fn new(host: Arc<str>, dial: Dial) -> Http {
        Http { host, dial, conn: None, pending: Vec::new() }
    }

    /// A kept-alive connection may have died quietly on the relay's idle timeout, so a failed request is sent once
    /// more, on a fresh connection.
    async fn request(
        &mut self,
        method: &str,
        path: &str,
        body: Option<(&str, &[u8])>,
        timeout: Duration,
    ) -> io::Result<(http1::Head, Vec<u8>)> {
        let mut last = io::Error::other("no request was sent");
        for _ in 0..2 {
            match tokio::time::timeout(timeout, self.send(method, path, body)).await {
                Ok(Ok(response)) => return Ok(response),
                Ok(Err(error)) => last = error,
                Err(_) => last = io::Error::new(io::ErrorKind::TimedOut, "the web proxy relay didn't answer in time"),
            }
        }
        Err(last)
    }

    async fn send(
        &mut self,
        method: &str,
        path: &str,
        body: Option<(&str, &[u8])>,
    ) -> io::Result<(http1::Head, Vec<u8>)> {
        // Taken out so a failure anywhere below leaves no half-used connection behind.
        let mut conn = match self.conn.take() {
            Some(conn) => conn,
            None => {
                self.pending.clear();
                (self.dial)().await?
            }
        };
        let (content_type, payload) = body.unwrap_or(("", &[][..]));
        let mut request = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: keep-alive\r\nContent-Length: {}\r\n",
            self.host,
            payload.len()
        );
        if !content_type.is_empty() {
            request.push_str(&format!("Content-Type: {content_type}\r\n"));
        }
        request.push_str("\r\n");
        let mut bytes = request.into_bytes();
        bytes.extend_from_slice(payload);
        conn.write_all(&bytes).await?;
        conn.flush().await?;
        let (head, answer) = http1::read_response(&mut conn, &mut self.pending, MAX_RESPONSE).await?;
        if !head.header("connection").is_some_and(|v| v.eq_ignore_ascii_case("close")) {
            self.conn = Some(conn);
        }
        Ok((head, answer))
    }
}

/// The uplink: frames go up in numbered posts the relay orders and de-duplicates by.
struct Link {
    up: Http,
    id: String,
    seq: u64,
}

impl Link {
    async fn post(&mut self, frames: &[u8]) -> io::Result<()> {
        let path = format!("{SESSION}/{}/up?seq={}", self.id, self.seq);
        self.seq += 1;
        let body = Some(("application/octet-stream", frames));
        let (head, _) = self.up.request("POST", &path, body, REQUEST_TIMEOUT).await?;
        match head.status {
            200 => Ok(()),
            status => Err(io::Error::other(format!("the web proxy relay refused an upload (HTTP {status})"))),
        }
    }

    /// Without it the relay keeps the session until its own idle timeout.
    async fn delete(&mut self) {
        let path = format!("{SESSION}/{}", self.id);
        let _ = self.up.request("DELETE", &path, None, REQUEST_TIMEOUT).await;
    }
}

type Batch = io::Result<Vec<Frame>>;

/// Aborts the task when dropped, so the long poll ends with the carrier.
struct Abort(JoinHandle<()>);

impl Drop for Abort {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Opens the session, says HELLO, waits for WELCOME, opens the stream and leaves a task carrying it.
async fn start(host: Arc<str>, dial: Dial, capability: &str) -> io::Result<DuplexStream> {
    let mut up = Http::new(host.clone(), dial.clone());
    let request = format!("{{\"bridge\":\"{capability}\"}}");
    let body = Some(("application/json", request.as_bytes()));
    let (head, answer) = up.request("POST", SESSION, body, REQUEST_TIMEOUT).await?;
    if head.status != 200 {
        return Err(io::Error::other(format!("the web proxy relay refused the session (HTTP {})", head.status)));
    }
    let (id, cursor) = session(&answer)?;
    let (tx, mut frames) = mpsc::channel(8);
    // The long poll runs before HELLO goes up: WELCOME comes back down it.
    let poll = Abort(tokio::spawn(poll(Http::new(host, dial), id.clone(), cursor, tx)));
    let mut link = Link { up, id, seq: 0 };
    match open_stream(&mut link, &mut frames).await {
        Ok(early) => {
            let (ours, theirs) = tokio::io::duplex(PIPE);
            let (from_client, to_client) = tokio::io::split(theirs);
            let carrier = Carrier {
                link,
                frames,
                from_client,
                to_client,
                outbox: VecDeque::new(),
                written: 0,
                send_window: INITIAL_WINDOW.into(),
                recv_window: INITIAL_WINDOW.into(),
                pending_grant: 0,
                grant_at: None,
                starved_until: None,
                _poll: poll,
            };
            tokio::spawn(carrier.run(early));
            Ok(ours)
        }
        Err(error) => {
            // The session exists on the relay by now, so it's deleted rather than left to time out.
            drop(poll);
            link.delete().await;
            Err(error)
        }
    }
}

/// HELLO, then WELCOME, then OPEN; returns the frames that came after WELCOME in its batch.
async fn open_stream(link: &mut Link, frames: &mut mpsc::Receiver<Batch>) -> io::Result<Vec<Frame>> {
    link.post(&frame(HELLO, CONTROL, &[1])).await?;
    let welcome = async {
        loop {
            let mut batch = frames.recv().await.ok_or_else(ended)??.into_iter();
            // Frames are taken in order, so a PING or BYE ahead of WELCOME in its batch is answered first.
            while let Some(f) = batch.next() {
                match (f.stream, f.kind) {
                    (CONTROL, WELCOME) => return Ok::<_, io::Error>(batch.collect()),
                    (CONTROL, PING) => link.post(&frame(PONG, CONTROL, &f.payload)).await?,
                    (CONTROL, BYE) => return Err(io::Error::other("the web proxy relay ended the session")),
                    _ => {}
                }
            }
        }
    };
    let early = tokio::time::timeout(WELCOME_TIMEOUT, welcome)
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "the web proxy relay didn't welcome the session"))??;
    link.post(&frame(OPEN, STREAM, &[])).await?;
    Ok(early)
}

fn ended() -> io::Error {
    io::Error::new(io::ErrorKind::UnexpectedEof, "the web proxy relay's downlink ended")
}

/// The long poll: the relay holds each request until it has frames or the wait runs out. Stops after handing on an
/// error, or once the carrier is gone.
async fn poll(mut down: Http, id: String, mut cursor: u64, tx: mpsc::Sender<Batch>) {
    loop {
        let path = format!("{SESSION}/{id}/down?cursor={cursor}&wait={}", LONG_POLL_WAIT.as_millis());
        let batch = match down.request("GET", &path, None, LONG_POLL_WAIT + LONG_POLL_MARGIN).await {
            Ok((head, _)) if head.status == 204 => continue,
            Ok((head, body)) if head.status == 200 => match head.header("x-cursor").map(str::parse::<u64>) {
                // Without moving the cursor on, the relay would send this batch again.
                Some(Ok(next)) => {
                    cursor = next;
                    parse_frames(&body)
                }
                Some(Err(_)) => Err(invalid("the web proxy relay sent a bad cursor")),
                None => parse_frames(&body),
            },
            Ok((head, _)) => {
                Err(io::Error::other(format!("the web proxy relay refused the downlink (HTTP {})", head.status)))
            }
            Err(error) => Err(error),
        };
        let failed = batch.is_err();
        if tx.send(batch).await.is_err() || failed {
            return;
        }
    }
}

/// Moves bytes between the client's end of the pipe and the relay until either side stops. Reading from the client
/// and writing to it are separate arms, so a client busy writing never stalls the bytes it's waiting to read.
struct Carrier {
    link: Link,
    frames: mpsc::Receiver<Batch>,
    from_client: ReadHalf<DuplexStream>,
    to_client: WriteHalf<DuplexStream>,
    // Downlink bytes not yet handed to the client, and how far into the first of them it got.
    outbox: VecDeque<Vec<u8>>,
    written: usize,
    // Bytes the relay will still take, and may still send.
    send_window: u64,
    recv_window: u64,
    // Downlink bytes handed on but not yet credited back.
    pending_grant: u32,
    grant_at: Option<Instant>,
    // Set when the uplink runs out of credit; cleared only when credit comes.
    starved_until: Option<Instant>,
    _poll: Abort,
}

impl Carrier {
    /// Whatever ends the carrier, what the relay sent before it ended is handed on if the client takes it in time,
    /// the pipe closes, so the client reads the end at once, and then the session is deleted.
    async fn run(mut self, early: Vec<Frame>) {
        let _ = self.serve(early).await;
        let flush = async {
            for (i, chunk) in self.outbox.iter().enumerate() {
                let at = if i == 0 { self.written } else { 0 };
                self.to_client.write_all(&chunk[at..]).await?;
            }
            io::Result::Ok(())
        };
        let _ = tokio::time::timeout(REQUEST_TIMEOUT, flush).await;
        let Carrier { mut link, frames, from_client, to_client, _poll, .. } = self;
        drop((frames, from_client, to_client, _poll));
        link.delete().await;
    }

    async fn serve(&mut self, early: Vec<Frame>) -> io::Result<()> {
        self.handle(early).await?;
        let mut buf = vec![0; UPLINK_FRAME];
        loop {
            let room = self.send_window.min(UPLINK_FRAME as u64) as usize;
            if room == 0 && self.starved_until.is_none() {
                self.starved_until = Some(Instant::now() + CREDIT_TIMEOUT);
            }
            let (grant_at, starved_until) = (self.grant_at, self.starved_until);
            tokio::select! {
                read = self.from_client.read(&mut buf[..room]), if room > 0 => {
                    let n = read?;
                    if n == 0 {
                        return Ok(());
                    }
                    self.send_window -= n as u64;
                    let mut body = frame(DATA, STREAM, &buf[..n]);
                    self.take_grant(&mut body);
                    self.link.post(&body).await?;
                }
                written = write_front(&mut self.to_client, &self.outbox, self.written), if !self.outbox.is_empty() => {
                    self.handed_on(written?).await?;
                }
                batch = self.frames.recv() => self.handle(batch.ok_or_else(ended)??).await?,
                () = tokio::time::sleep_until(grant_at.unwrap_or_else(Instant::now)), if grant_at.is_some() => {
                    let mut body = Vec::new();
                    self.take_grant(&mut body);
                    if !body.is_empty() {
                        self.link.post(&body).await?;
                    }
                }
                () = tokio::time::sleep_until(starved_until.unwrap_or_else(Instant::now)), if room == 0 => {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "the web proxy relay gave no uplink credit"));
                }
            }
        }
    }

    /// `n` more bytes of the outbox reached the client's end of the pipe: that's where their credit goes back.
    async fn handed_on(&mut self, n: usize) -> io::Result<()> {
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::WriteZero, "the client's end of the web proxy pipe closed"));
        }
        self.written += n;
        if self.outbox.front().is_some_and(|front| self.written == front.len()) {
            self.outbox.pop_front();
            self.written = 0;
        }
        self.pending_grant = self.pending_grant.saturating_add(n as u32);
        if self.pending_grant >= GRANT_THRESHOLD {
            let mut body = Vec::new();
            self.take_grant(&mut body);
            self.link.post(&body).await?;
        } else if self.grant_at.is_none() {
            self.grant_at = Some(Instant::now() + GRANT_DELAY);
        }
        Ok(())
    }

    async fn handle(&mut self, frames: Vec<Frame>) -> io::Result<()> {
        for f in frames {
            match (f.stream, f.kind) {
                (CONTROL, PING) => self.link.post(&frame(PONG, CONTROL, &f.payload)).await?,
                (CONTROL, BYE) => return Err(io::Error::other("the web proxy relay ended the session")),
                // An empty DATA frame carries nothing to hand on or credit back.
                (STREAM, DATA) if f.payload.is_empty() => {}
                (STREAM, DATA) => {
                    let len = f.payload.len() as u64;
                    self.recv_window = self
                        .recv_window
                        .checked_sub(len)
                        .ok_or_else(|| invalid("the web proxy relay sent past its credit"))?;
                    self.outbox.push_back(f.payload);
                }
                (STREAM, WINDOW) => {
                    let credit = <[u8; 4]>::try_from(f.payload.as_slice())
                        .map_err(|_| invalid("the web proxy relay sent a bad WINDOW frame"))?;
                    self.send_window += u64::from(u32::from_be_bytes(credit));
                    if self.send_window > 0 {
                        self.starved_until = None;
                    }
                }
                (STREAM, CLOSE) => return Err(io::Error::other("the web proxy relay closed the stream")),
                _ => {}
            }
        }
        Ok(())
    }

    /// Appends the WINDOW frame that credits back what was handed on, if anything was.
    fn take_grant(&mut self, body: &mut Vec<u8>) {
        if self.pending_grant > 0 {
            push_frame(body, WINDOW, STREAM, &self.pending_grant.to_be_bytes());
            self.recv_window += u64::from(self.pending_grant);
            self.pending_grant = 0;
        }
        self.grant_at = None;
    }
}

/// Writes what it can of the outbox's first buffer, from `at` on.
async fn write_front(to: &mut WriteHalf<DuplexStream>, outbox: &VecDeque<Vec<u8>>, at: usize) -> io::Result<usize> {
    match outbox.front() {
        Some(front) => to.write(&front[at..]).await,
        None => Ok(0),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
    use tokio::sync::Notify;

    use super::*;

    fn secret(hex: &str) -> Secret {
        hex.parse().unwrap()
    }

    #[test]
    fn bridge_capability_matches_the_published_vectors() {
        let plain = secret("000102030405060708090a0b0c0d0e0f");
        let padded = secret("dd000102030405060708090a0b0c0d0e0f");
        assert_eq!(bridge_capability("proxy.example.com", &plain), "MHLEY5PmW1GWqJkSrlmJpvJUiLhBH_QKy6yKg8a0JPk");
        assert_eq!(bridge_capability("proxy.example.com", &padded), "IpJrt3e7sKtzPyoXy6w-Zj6GGEvsvclN66JzQEfPYLA");
    }

    #[test]
    fn host_names_are_made_canonical_or_refused() {
        assert_eq!(canonical_host(" Relay.Example.COM. ").unwrap(), "relay.example.com");
        for bad in ["", "relay", "1.2.3.4", "::1", "re lay.example.com", "-a.example.com", "relé.example.com", "a..b"]
        {
            assert!(canonical_host(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn frames_round_trip_and_bad_batches_are_refused() {
        let mut body = frame(HELLO, CONTROL, &[1]);
        push_frame(&mut body, DATA, 0x00ab_cdef, b"xyz");
        let frames = parse_frames(&body).unwrap();
        assert_eq!(frames[1], Frame { kind: DATA, stream: 0x00ab_cdef, payload: b"xyz".to_vec() });
        // The HELLO frame is 8 header bytes and 1 payload byte; the DATA header follows.
        assert_eq!(&body[9..13], &[DATA, 0xab, 0xcd, 0xef]);
        let mut oversize = frame(DATA, STREAM, &[]);
        oversize[4..8].copy_from_slice(&(MAX_PAYLOAD as u32 + 1).to_be_bytes());
        for bad in [&b""[..], &body[..body.len() - 1], &[0x7f, 0, 0, 0, 0, 0, 0, 0], &oversize] {
            assert!(parse_frames(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn reads_the_session_answer() {
        assert_eq!(session(br#"{"id": "s-1_a", "cursor": 7}"#).unwrap(), ("s-1_a".to_owned(), 7));
        assert_eq!(session(br#"{"cursor":0,"id":"abc"}"#).unwrap(), ("abc".to_owned(), 0));
        assert_eq!(session(br#"{"id":"abc"}"#).unwrap(), ("abc".to_owned(), 0));
        for bad in [&br#"{"id": "a/b"}"#[..], br#"{"id": ""}"#, br#"{"cursor": 1}"#, br#"{"id": "a", "cursor": "x"}"#] {
            assert!(session(bad).is_err());
        }
    }

    // A relay in memory: it welcomes HELLO, echoes DATA when `echo` is on, and logs every frame and post number.
    #[derive(Default)]
    struct Relay {
        echo: bool,
        // Answers HELLO with BYE instead of WELCOME.
        refuse: bool,
        down: VecDeque<u8>,
        seen: Vec<Frame>,
        seqs: Vec<u64>,
        deleted: bool,
    }

    type Shared = Arc<(Mutex<Relay>, Notify)>;

    fn dial(shared: &Shared) -> Dial {
        let shared = shared.clone();
        Arc::new(move || {
            let shared = shared.clone();
            Box::pin(async move {
                let (client, server) = tokio::io::duplex(1 << 20);
                tokio::spawn(serve(server, shared));
                Ok(Box::new(client) as Box<dyn Stream>)
            })
        })
    }

    async fn serve(mut conn: DuplexStream, shared: Shared) {
        let mut pending = Vec::new();
        while let Some((method, path, body)) = read_request(&mut conn, &mut pending).await {
            let (status, cursor, answer) = answer(&shared, &method, &path, &body).await;
            let mut out = format!("HTTP/1.1 {status} X\r\n").into_bytes();
            match cursor {
                // Batches come chunked, as a reverse proxy in front of the relay sends them.
                Some(cursor) => {
                    out.extend(format!("Transfer-Encoding: chunked\r\nx-cursor: {cursor}\r\n\r\n").bytes());
                    out.extend(format!("{:x}\r\n", answer.len()).bytes());
                    out.extend(&answer);
                    out.extend(b"\r\n0\r\n\r\n");
                }
                None if status == 204 => out.extend(b"\r\n"),
                None => {
                    out.extend(format!("Content-Length: {}\r\n\r\n", answer.len()).bytes());
                    out.extend(&answer);
                }
            }
            if conn.write_all(&out).await.is_err() {
                return;
            }
        }
    }

    async fn read_request(conn: &mut DuplexStream, pending: &mut Vec<u8>) -> Option<(String, String, Vec<u8>)> {
        let mut chunk = [0; 4096];
        let end = loop {
            if let Some(at) = pending.windows(4).position(|w| w == b"\r\n\r\n") {
                break at;
            }
            let n = conn.read(&mut chunk).await.ok().filter(|&n| n > 0)?;
            pending.extend_from_slice(&chunk[..n]);
        };
        let head = String::from_utf8(pending.drain(..end + 4).collect()).ok()?;
        let mut lines = head.lines();
        let mut first = lines.next()?.split(' ');
        let (method, path) = (first.next()?.to_owned(), first.next()?.to_owned());
        let length = lines
            .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().ok()))
            .flatten()
            .unwrap_or(0);
        while pending.len() < length {
            let n = conn.read(&mut chunk).await.ok().filter(|&n| n > 0)?;
            pending.extend_from_slice(&chunk[..n]);
        }
        Some((method, path, pending.drain(..length).collect()))
    }

    async fn answer(shared: &Shared, method: &str, path: &str, body: &[u8]) -> (u16, Option<u64>, Vec<u8>) {
        let (relay, wake) = &**shared;
        match method {
            "POST" if path == SESSION => {
                assert!(String::from_utf8_lossy(body).contains(r#"{"bridge":""#), "{body:?}");
                (200, None, br#"{"id": "s1", "cursor": 5}"#.to_vec())
            }
            "POST" => {
                let seq = path.rsplit("seq=").next().unwrap().parse().unwrap();
                let mut relay = relay.lock().unwrap();
                relay.seqs.push(seq);
                for f in parse_frames(body).unwrap() {
                    match (f.stream, f.kind) {
                        (CONTROL, HELLO) => {
                            let answer = if relay.refuse { BYE } else { WELCOME };
                            relay.down.extend(frame(answer, CONTROL, &[1]));
                        }
                        (STREAM, DATA) if relay.echo => relay.down.extend(frame(DATA, STREAM, &f.payload)),
                        _ => {}
                    }
                    relay.seen.push(f);
                }
                wake.notify_one();
                (200, None, Vec::new())
            }
            "GET" => {
                let cursor: u64 = path.split("cursor=").nth(1).unwrap().split('&').next().unwrap().parse().unwrap();
                for _ in 0..2 {
                    let batch: Vec<u8> = relay.lock().unwrap().down.drain(..).collect();
                    if !batch.is_empty() {
                        return (200, Some(cursor + 1), batch);
                    }
                    let _ = tokio::time::timeout(Duration::from_millis(50), wake.notified()).await;
                }
                (204, None, Vec::new())
            }
            _ => {
                relay.lock().unwrap().deleted = true;
                (204, None, Vec::new())
            }
        }
    }

    async fn carrier(echo: bool) -> (DuplexStream, Shared) {
        let shared: Shared = Arc::default();
        shared.0.lock().unwrap().echo = echo;
        let stream = start("relay.example.com".into(), dial(&shared), "cap").await.unwrap();
        (stream, shared)
    }

    async fn until(shared: &Shared, done: impl Fn(&Relay) -> bool) {
        for _ in 0..200 {
            if done(&shared.0.lock().unwrap()) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("the relay never got there");
    }

    #[tokio::test]
    async fn carries_bytes_both_ways_and_ends_the_session() {
        let (mut stream, shared) = carrier(true).await;
        stream.write_all(b"ping me").await.unwrap();
        let mut echoed = [0; 7];
        stream.read_exact(&mut echoed).await.unwrap();
        assert_eq!(&echoed, b"ping me");
        drop(stream);
        until(&shared, |r| r.deleted).await;
        let relay = shared.0.lock().unwrap();
        let kinds: Vec<_> = relay.seen.iter().map(|f| (f.stream, f.kind)).collect();
        assert_eq!(kinds[..3], [(CONTROL, HELLO), (STREAM, OPEN), (STREAM, DATA)]);
        // Posts are numbered from 0, one apart.
        assert!(relay.seqs.iter().enumerate().all(|(i, &seq)| seq == i as u64), "{:?}", relay.seqs);
    }

    #[tokio::test]
    async fn downlink_credit_is_granted_back() {
        let (mut stream, shared) = carrier(false).await;
        {
            let (relay, wake) = &*shared;
            relay.lock().unwrap().down.extend(frame(DATA, STREAM, &vec![7; 300 << 10]));
            wake.notify_one();
        }
        let mut got = vec![0; 300 << 10];
        stream.read_exact(&mut got).await.unwrap();
        // Credit goes back as the bytes reach the pipe, in grants that add up to all of them.
        let granted = |r: &Relay| -> u32 {
            let windows = r.seen.iter().filter(|f| (f.stream, f.kind) == (STREAM, WINDOW));
            windows.map(|f| u32::from_be_bytes(f.payload[..4].try_into().unwrap())).sum()
        };
        until(&shared, |r| granted(r) == 300 << 10).await;
    }

    #[tokio::test]
    async fn bye_ends_the_stream_and_the_session() {
        let (mut stream, shared) = carrier(false).await;
        {
            let (relay, wake) = &*shared;
            let mut relay = relay.lock().unwrap();
            relay.down.extend(frame(DATA, STREAM, b"last"));
            relay.down.extend(frame(BYE, CONTROL, &[]));
            wake.notify_one();
        }
        // What came before BYE still arrives, then the stream ends.
        let mut got = Vec::new();
        stream.read_to_end(&mut got).await.unwrap();
        assert_eq!(got, b"last");
        until(&shared, |r| r.deleted).await;
    }

    #[tokio::test]
    async fn a_big_write_and_a_big_read_together_dont_deadlock() {
        // The client writes 2 MiB before it reads anything, while the relay echoes it all back: the carrier has to
        // keep reading from the client while the echo waits for it.
        let (stream, _shared) = carrier(true).await;
        let (mut read, mut write) = tokio::io::split(stream);
        let data: Vec<u8> = (0..2 << 20).map(|i: u32| i as u8).collect();
        let both = async {
            write.write_all(&data).await.unwrap();
            let mut back = vec![0; data.len()];
            read.read_exact(&mut back).await.unwrap();
            back
        };
        let back = tokio::time::timeout(Duration::from_secs(30), both).await.expect("the carrier deadlocked");
        assert!(back == data, "the echo came back changed");
    }

    #[tokio::test]
    async fn a_session_that_fails_to_open_is_deleted() {
        let shared: Shared = Arc::default();
        shared.0.lock().unwrap().refuse = true;
        assert!(start("relay.example.com".into(), dial(&shared), "cap").await.is_err());
        assert!(shared.0.lock().unwrap().deleted);
    }
}
