//! MTProto over WebSocket (<https://core.telegram.org/mtproto/transports#websocket>): binary frames whose payloads
//! form one byte stream, which the obfuscated transport then runs over.

use std::collections::VecDeque;
use std::future::poll_fn;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, ready};

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;
use tokio::sync::Notify;
use tokio::sync::futures::Notified;

use super::proxy::Target;
use super::{Options, Stream, dial_any, http1, tls};
use crate::crypto::{random, sha1};
use crate::errors::{Error, Result};
use crate::session::internals::data_center::Dc;
use crate::utils::base64_encode;

/// Room a frame header needs in front of its payload: 2 bytes, an 8-byte length and a 4-byte mask at most.
pub(super) const ROOM: usize = 14;
const ACCEPT_GUID: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
// MTProto packets are at most 16 MiB, so no frame needs to be longer.
const MAX_FRAME: u64 = 16 << 20;
const CONTINUATION: u8 = 0;
const TEXT: u8 = 1;
const BINARY: u8 = 2;
const CLOSE: u8 = 8;
const PING: u8 = 9;
const PONG: u8 = 10;
// Pongs waiting for the writer. A server that pings faster than they go out loses the oldest, so memory stays bounded.
const MAX_PONGS: usize = 8;

/// The read half of a WebSocket over a dialed stream.
pub(super) type Read = Reader<tokio::io::ReadHalf<Box<dyn Stream>>>;
/// The write half of a WebSocket over a dialed stream.
pub(super) type Write = Writer<tokio::io::WriteHalf<Box<dyn Stream>>>;

/// Ping payloads the reader saw, oldest first, waiting for the writer to send them back as pongs; `ready` wakes the
/// writer's owner, so an idle connection answers too.
#[derive(Default)]
struct Pongs {
    queue: Mutex<VecDeque<Vec<u8>>>,
    ready: Notify,
}

impl Pongs {
    fn push(&self, payload: &[u8]) {
        let mut queue = self.queue.lock().unwrap();
        // The oldest answer makes room, and lends its buffer.
        let mut pong = if queue.len() == MAX_PONGS { queue.pop_front().unwrap_or_default() } else { Vec::new() };
        pong.clear();
        pong.extend_from_slice(payload);
        queue.push_back(pong);
        drop(queue);
        self.ready.notify_one();
    }
}

type Pong = Arc<Pongs>;

/// Where a data center's WebSocket listens.
pub(super) struct Endpoint {
    host: String,
    port: u16,
    path: &'static str,
    tls: bool,
}

impl Endpoint {
    /// Telegram's: TLS on port 443 at the data center's name, the only form its certificates cover.
    pub(super) fn of(dc: &Dc) -> Result<Endpoint> {
        let name = match dc.id {
            1 => "pluto",
            2 => "venus",
            3 => "aurora",
            4 => "vesta",
            5 => "flora",
            id => return Err(Error::InvalidDc(id)),
        };
        let path = if dc.test { "/apiws_test" } else { "/apiws" };
        Ok(Endpoint { host: format!("{name}.web.telegram.org"), port: 443, path, tls: true })
    }

    #[cfg(test)]
    pub(super) fn plain(addr: std::net::SocketAddr) -> Endpoint {
        Endpoint { host: addr.ip().to_string(), port: addr.port(), path: "/apiws", tls: false }
    }
}

/// Dials the endpoint the way the options say (through the proxy, the connection factory or plain TCP), then does
/// TLS and the WebSocket handshake.
pub(super) async fn open(endpoint: &Endpoint, options: &Options) -> Result<(Read, Write)> {
    let mut stream = dial(endpoint, options).await?;
    if endpoint.tls {
        stream = Box::new(tls::connect(stream, &endpoint.host).await?);
    }
    Ok(handshake(stream, &endpoint.host, endpoint.path).await?)
}

async fn dial(endpoint: &Endpoint, options: &Options) -> Result<Box<dyn Stream>> {
    let host = (endpoint.host.as_str(), endpoint.port);
    let factory = options.connection_factory.as_deref();
    let Some(proxy) = &options.proxy else {
        return Ok(match factory {
            Some(factory) => dial_any(factory, host).await?,
            None => tcp(host).await?,
        });
    };
    let mut stream = match factory {
        Some(factory) => dial_any(factory, proxy.endpoint()).await?,
        None => tcp(proxy.endpoint()).await?,
    };
    // The proxy is given the host name and looks it up itself (SOCKS4, which can't, excepted), so this side does no
    // DNS for it. A proxy that refuses the name fails the connect: it's never retried with a local lookup.
    proxy.handshake(&mut stream, Target::Domain(&endpoint.host, endpoint.port)).await?;
    Ok(stream)
}

async fn tcp(host: (&str, u16)) -> io::Result<Box<dyn Stream>> {
    let stream = TcpStream::connect(host).await?;
    stream.set_nodelay(true)?;
    Ok(Box::new(stream))
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn accept_key(key: &str) -> String {
    base64_encode(&sha1(&[key.as_bytes(), ACCEPT_GUID]))
}

/// Upgrades `stream` to a WebSocket that speaks Telegram's `binary` subprotocol, and splits it.
pub(super) async fn handshake<S: AsyncRead + AsyncWrite + Unpin>(
    mut stream: S,
    host: &str,
    path: &str,
) -> io::Result<(Reader<tokio::io::ReadHalf<S>>, Writer<tokio::io::WriteHalf<S>>)> {
    let key = base64_encode(&random::<16>());
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\n\
         Sec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: binary\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).await?;
    stream.flush().await?;
    let (head, early) = http1::read_head(&mut stream).await?;
    if head.status != 101 {
        return Err(invalid(format!("the WebSocket upgrade was answered with HTTP {}", head.status)));
    }
    let upgrade = head.header("upgrade").is_some_and(|v| v.eq_ignore_ascii_case("websocket"));
    let connection =
        head.header("connection").is_some_and(|v| v.split(',').any(|t| t.trim().eq_ignore_ascii_case("upgrade")));
    if !upgrade || !connection {
        return Err(invalid("the server didn't switch to WebSocket"));
    }
    if head.header("sec-websocket-accept") != Some(accept_key(&key).as_str()) {
        return Err(invalid("the server's WebSocket accept key doesn't match"));
    }
    Ok(split(stream, early))
}

fn split<S: AsyncRead + AsyncWrite>(
    stream: S,
    early: Vec<u8>,
) -> (Reader<tokio::io::ReadHalf<S>>, Writer<tokio::io::WriteHalf<S>>) {
    let (read, write) = tokio::io::split(stream);
    let pong = Pong::default();
    (Reader::new(read, early, pong.clone()), Writer { inner: write, out: Vec::new(), sent: 0, pong })
}

/// The header of a masked client frame: its bytes and how many of them are used.
fn header(opcode: u8, len: usize, mask: [u8; 4]) -> ([u8; ROOM], usize) {
    let mut head = [0; ROOM];
    head[0] = 0x80 | opcode;
    let at = match len {
        0..=125 => {
            head[1] = 0x80 | len as u8;
            2
        }
        126..=0xffff => {
            head[1] = 0x80 | 126;
            head[2..4].copy_from_slice(&(len as u16).to_be_bytes());
            4
        }
        _ => {
            head[1] = 0x80 | 127;
            head[2..10].copy_from_slice(&(len as u64).to_be_bytes());
            10
        }
    };
    head[at..at + 4].copy_from_slice(&mask);
    (head, at + 4)
}

fn apply_mask(payload: &mut [u8], mask: [u8; 4]) {
    for (byte, key) in payload.iter_mut().zip(mask.iter().cycle()) {
        *byte ^= key;
    }
}

/// Reads give the payloads of binary frames in order. A ping is handed to the writer, a close frame reads as the
/// end, and anything a server mustn't send is `InvalidData`.
pub(super) struct Reader<R> {
    inner: R,
    // What arrived with the upgrade answer, read before anything else.
    early: Vec<u8>,
    early_read: usize,
    head: [u8; 10],
    head_got: usize,
    // Payload bytes still to come in the current data frame.
    left: u64,
    control: Option<Control>,
    pong: Pong,
    closed: bool,
}

struct Control {
    opcode: u8,
    payload: [u8; 125],
    len: usize,
    got: usize,
}

impl<R: AsyncRead + Unpin> Reader<R> {
    fn new(inner: R, early: Vec<u8>, pong: Pong) -> Self {
        Reader { inner, early, early_read: 0, head: [0; 10], head_got: 0, left: 0, control: None, pong, closed: false }
    }

    fn read_raw(&mut self, cx: &mut Context<'_>, dst: &mut [u8]) -> Poll<io::Result<usize>> {
        if self.early_read < self.early.len() {
            let n = dst.len().min(self.early.len() - self.early_read);
            dst[..n].copy_from_slice(&self.early[self.early_read..self.early_read + n]);
            self.early_read += n;
            return Poll::Ready(Ok(n));
        }
        let mut buf = ReadBuf::new(dst);
        ready!(Pin::new(&mut self.inner).poll_read(cx, &mut buf))?;
        Poll::Ready(Ok(buf.filled().len()))
    }

    /// Reads a frame header; `false` when the stream ended cleanly between frames.
    fn poll_header(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<bool>> {
        loop {
            let need = match self.head_got {
                0 | 1 => 2,
                _ => match self.head[1] & 0x7f {
                    126 => 4,
                    127 => 10,
                    _ => 2,
                },
            };
            if self.head_got >= need {
                break;
            }
            let (got, mut head) = (self.head_got, self.head);
            let n = ready!(self.read_raw(cx, &mut head[got..need]))?;
            self.head[got..need].copy_from_slice(&head[got..need]);
            if n == 0 {
                return match got {
                    0 => Poll::Ready(Ok(false)),
                    _ => {
                        Poll::Ready(Err(io::Error::new(io::ErrorKind::UnexpectedEof, "the WebSocket ended in a frame")))
                    }
                };
            }
            self.head_got += n;
        }
        let (first, second) = (self.head[0], self.head[1]);
        let len = match second & 0x7f {
            126 => u64::from(u16::from_be_bytes([self.head[2], self.head[3]])),
            127 => u64::from_be_bytes(self.head[2..10].try_into().unwrap_or_default()),
            len => u64::from(len),
        };
        self.head_got = 0;
        if first & 0x70 != 0 {
            return Poll::Ready(Err(invalid("a WebSocket frame set reserved bits")));
        }
        if second & 0x80 != 0 {
            return Poll::Ready(Err(invalid("the server masked a WebSocket frame")));
        }
        let fin = first & 0x80 != 0;
        match first & 0x0f {
            CONTINUATION | BINARY if len > MAX_FRAME => Poll::Ready(Err(invalid("a WebSocket frame is too long"))),
            CONTINUATION | BINARY => {
                self.left = len;
                Poll::Ready(Ok(true))
            }
            TEXT => Poll::Ready(Err(invalid("the server sent a WebSocket text frame"))),
            opcode @ (CLOSE | PING | PONG) if fin && len <= 125 => {
                self.control = Some(Control { opcode, payload: [0; 125], len: len as usize, got: 0 });
                Poll::Ready(Ok(true))
            }
            _ => Poll::Ready(Err(invalid("the server sent an unknown, fragmented or oversized WebSocket frame"))),
        }
    }

    fn poll_control(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        while let Some(control) = &self.control {
            let (got, len, mut payload) = (control.got, control.len, control.payload);
            if got == len {
                match self.control.take().map(|c| c.opcode) {
                    Some(CLOSE) => self.closed = true,
                    Some(PING) => self.pong.push(&payload[..len]),
                    _ => {}
                }
                break;
            }
            let n = ready!(self.read_raw(cx, &mut payload[got..len]))?;
            if n == 0 {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "the WebSocket ended in a frame",
                )));
            }
            if let Some(control) = &mut self.control {
                control.payload[got..got + n].copy_from_slice(&payload[got..got + n]);
                control.got += n;
            }
        }
        Poll::Ready(Ok(()))
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for Reader<R> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        loop {
            if this.closed {
                return Poll::Ready(Ok(()));
            }
            if this.control.is_some() {
                ready!(this.poll_control(cx))?;
                continue;
            }
            if this.left == 0 {
                if !ready!(this.poll_header(cx))? {
                    return Poll::Ready(Ok(()));
                }
                continue;
            }
            if buf.remaining() == 0 {
                return Poll::Ready(Ok(()));
            }
            let n = this.left.min(buf.remaining() as u64) as usize;
            let read = ready!(this.read_raw(cx, buf.initialize_unfilled_to(n)))?;
            if read == 0 {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "the WebSocket ended in a frame",
                )));
            }
            buf.advance(read);
            this.left -= read as u64;
            return Poll::Ready(Ok(()));
        }
    }
}

/// Sends masked binary frames, answering the pings the reader saw before the next one, or alone with
/// [`Writer::flush_control`]. Packets go through [`Writer::send_in_place`] without a copy; plain writes (the
/// obfuscation header) are copied into a frame.
pub(super) struct Writer<W> {
    inner: W,
    out: Vec<u8>,
    sent: usize,
    pong: Pong,
}

impl<W: AsyncWrite + Unpin> Writer<W> {
    /// Adds a frame holding a copy of `payload` to the output.
    fn queue(&mut self, opcode: u8, payload: &[u8]) {
        let mask = random::<4>();
        let (head, len) = header(opcode, payload.len(), mask);
        self.out.extend_from_slice(&head[..len]);
        let start = self.out.len();
        self.out.extend_from_slice(payload);
        apply_mask(&mut self.out[start..], mask);
    }

    /// Queues a pong for every ping the reader saw since the last call.
    fn queue_pong(&mut self) {
        let pongs = std::mem::take(&mut *self.pong.queue.lock().unwrap());
        for pong in pongs {
            self.queue(PONG, &pong);
        }
    }

    fn poll_drain(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        while self.sent < self.out.len() {
            let n = ready!(Pin::new(&mut self.inner).poll_write(cx, &self.out[self.sent..]))?;
            if n == 0 {
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
            }
            self.sent += n;
        }
        self.out.clear();
        self.sent = 0;
        Poll::Ready(Ok(()))
    }

    /// Sends `buf[start..]` as one binary frame: the header goes into the [`ROOM`] bytes before `start`, and the
    /// payload is masked where it lies.
    pub(super) async fn send_in_place(&mut self, buf: &mut [u8], start: usize) -> io::Result<()> {
        self.queue_pong();
        poll_fn(|cx| self.poll_drain(cx)).await?;
        let mask = random::<4>();
        let (head, len) = header(BINARY, buf.len() - start, mask);
        let from = start - len;
        buf[from..start].copy_from_slice(&head[..len]);
        apply_mask(&mut buf[start..], mask);
        self.inner.write_all(&buf[from..]).await?;
        self.inner.flush().await
    }

    /// Completes once the reader has seen a ping since the last wake. A wake whose pongs already went out with a
    /// data frame is harmless: [`Writer::flush_control`] then has nothing to send.
    pub(super) fn control_ready(&self) -> Notified<'_> {
        self.pong.ready.notified()
    }

    /// Sends the pongs waiting, without a data frame, so a connection with nothing to send still answers pings.
    pub(super) async fn flush_control(&mut self) -> io::Result<()> {
        self.queue_pong();
        poll_fn(|cx| self.poll_drain(cx)).await?;
        self.inner.flush().await
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for Writer<W> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }
        ready!(this.poll_drain(cx))?;
        this.queue_pong();
        this.queue(BINARY, buf);
        // The frame is taken whole; what the socket doesn't take now goes out on the next write or flush.
        if let Poll::Ready(Err(e)) = this.poll_drain(cx) {
            return Poll::Ready(Err(e));
        }
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        ready!(this.poll_drain(cx))?;
        this.queue_pong();
        ready!(this.poll_drain(cx))?;
        Pin::new(&mut this.inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        ready!(this.poll_drain(cx))?;
        Pin::new(&mut this.inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream};
    use tokio::net::TcpListener;

    use super::{
        BINARY, CLOSE, CONTINUATION, Dc, Endpoint, MAX_PONGS, PING, PONG, ROOM, Reader, TEXT, Writer, accept_key,
        dial, handshake, split,
    };
    use crate::connection::{Connection, Options, Proxy};
    use crate::crypto::aes;
    use crate::raw::{Identifiable, Serializable, functions, types};
    use crate::session::test_dc::TestDc;

    /// A frame as a server sends it: unmasked, `fin` unless it's a fragment.
    fn server_frame(fin: bool, opcode: u8, payload: &[u8]) -> Vec<u8> {
        let mut frame = vec![if fin { 0x80 } else { 0 } | opcode];
        match payload.len() {
            len @ 0..=125 => frame.push(len as u8),
            len @ 126..=0xffff => {
                frame.push(126);
                frame.extend_from_slice(&(len as u16).to_be_bytes());
            }
            len => {
                frame.push(127);
                frame.extend_from_slice(&(len as u64).to_be_bytes());
            }
        }
        frame.extend_from_slice(payload);
        frame
    }

    /// Reads one client frame, checks it's masked and final, and gives its opcode and unmasked payload.
    async fn client_frame(stream: &mut (impl AsyncRead + Unpin)) -> (u8, Vec<u8>) {
        let mut head = [0; 2];
        stream.read_exact(&mut head).await.unwrap();
        assert_eq!((head[0] & 0xf0, head[1] & 0x80), (0x80, 0x80), "not a final masked frame: {head:?}");
        let len = match head[1] & 0x7f {
            126 => u64::from(stream.read_u16().await.unwrap()),
            127 => stream.read_u64().await.unwrap(),
            len => u64::from(len),
        };
        let mut mask = [0; 4];
        stream.read_exact(&mut mask).await.unwrap();
        let mut payload = vec![0; len as usize];
        stream.read_exact(&mut payload).await.unwrap();
        payload.iter_mut().zip(mask.iter().cycle()).for_each(|(b, m)| *b ^= m);
        (head[0] & 0x0f, payload)
    }

    type Halves = (Reader<tokio::io::ReadHalf<DuplexStream>>, Writer<tokio::io::WriteHalf<DuplexStream>>);

    fn pair() -> (Halves, DuplexStream) {
        let (ours, theirs) = tokio::io::duplex(1 << 20);
        (split(ours, Vec::new()), theirs)
    }

    /// Reads an upgrade request and answers it as a WebSocket server does, with `accept` as the key it sends back
    /// (the right one when `None`) and `after` in the same write. Returns the request head.
    async fn answer_upgrade(
        stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
        accept: Option<&str>,
        after: &[u8],
    ) -> String {
        let mut head = Vec::new();
        while !head.ends_with(b"\r\n\r\n") {
            head.push(stream.read_u8().await.unwrap());
        }
        let head = String::from_utf8(head).unwrap();
        let key = head.lines().find_map(|l| l.strip_prefix("Sec-WebSocket-Key: ")).unwrap();
        let accept = accept.map_or_else(|| accept_key(key), str::to_owned);
        let answer = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
        );
        stream.write_all(&[answer.as_bytes(), after].concat()).await.unwrap();
        head
    }

    #[tokio::test]
    async fn bytes_after_101_head_are_kept() {
        let (client, mut server) = tokio::io::duplex(1 << 16);
        let server =
            tokio::spawn(async move { answer_upgrade(&mut server, None, &server_frame(true, BINARY, b"early")).await });
        let (mut reader, _writer) = handshake(client, "venus.web.telegram.org", "/apiws").await.unwrap();
        let mut early = [0; 5];
        reader.read_exact(&mut early).await.unwrap();
        assert_eq!(&early, b"early");
        let head = server.await.unwrap();
        assert!(head.starts_with("GET /apiws HTTP/1.1\r\nHost: venus.web.telegram.org\r\n"), "{head}");
        assert!(head.contains("\r\nSec-WebSocket-Protocol: binary\r\n") && head.contains("\r\nUpgrade: websocket\r\n"));
    }

    #[tokio::test]
    async fn a_wrong_accept_key_is_refused() {
        let (client, mut server) = tokio::io::duplex(1 << 16);
        tokio::spawn(async move { answer_upgrade(&mut server, Some("bm90IGl0"), b"").await });
        let refused = handshake(client, "venus.web.telegram.org", "/apiws").await.err().map(|e| e.kind());
        assert_eq!(refused, Some(std::io::ErrorKind::InvalidData));
    }

    /// Stands in for Telegram's WebSocket front: takes the obfuscated stream out of client frames and passes it to
    /// `dc` as the plain intermediate transport, and back.
    async fn relay(listener: TcpListener, dc: std::net::SocketAddr) {
        let (mut client, _) = listener.accept().await.unwrap();
        answer_upgrade(&mut client, None, b"").await;
        let (opcode, header) = client_frame(&mut client).await;
        assert_eq!((opcode, header.len()), (BINARY, 64));
        // The client encrypts with key and iv from its header, and decrypts with the same bytes reversed.
        let up: ([u8; 32], [u8; 16]) = (header[8..40].try_into().unwrap(), header[40..56].try_into().unwrap());
        let mut reversed = header[8..56].to_vec();
        reversed.reverse();
        let down: ([u8; 32], [u8; 16]) = (reversed[..32].try_into().unwrap(), reversed[32..].try_into().unwrap());
        let (mut up_iv, mut up_used, mut down_iv, mut down_used) = (up.1, 0, down.1, 0);
        let mut plain = header;
        aes::ctr256(&mut plain, &up.0, &mut up_iv, &mut up_used);
        assert_eq!(plain[56..60], [0xee; 4]);
        let mut dc = tokio::net::TcpStream::connect(dc).await.unwrap();
        dc.write_all(&plain[56..60]).await.unwrap();
        let (mut client_read, mut client_write) = client.into_split();
        let (mut dc_read, mut dc_write) = dc.into_split();
        let upstream = async move {
            loop {
                let (_, mut bytes) = client_frame(&mut client_read).await;
                aes::ctr256(&mut bytes, &up.0, &mut up_iv, &mut up_used);
                dc_write.write_all(&bytes).await.unwrap();
            }
        };
        let downstream = async move {
            let mut buf = vec![0; 1 << 16];
            loop {
                let n = dc_read.read(&mut buf).await.unwrap();
                if n == 0 {
                    return;
                }
                aes::ctr256(&mut buf[..n], &down.0, &mut down_iv, &mut down_used);
                client_write.write_all(&server_frame(true, BINARY, &buf[..n])).await.unwrap();
            }
        };
        tokio::select! {
            _ = upstream => {}
            _ = downstream => {}
        }
    }

    #[tokio::test]
    async fn mtproto_over_a_ws_relay_to_testdc() {
        let test_dc = TestDc::start([5; 256], false).await;
        let dc = test_dc.dc(2);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let at = listener.local_addr().unwrap();
        tokio::spawn(relay(listener, dc.addr));
        let mut connection = Connection::open_websocket(&dc, &Options::default(), &Endpoint::plain(at)).await.unwrap();

        // An unencrypted req_pq_multi, as a key exchange starts: the data center answers with its resPQ.
        let nonce = [7; 16];
        let body = functions::ReqPqMulti { nonce }.to_bytes();
        let msg_id =
            (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64) << 32;
        let packet = [&[0; 8][..], &msg_id.to_le_bytes(), &(body.len() as u32).to_le_bytes(), &body].concat();
        connection.send(&packet).await.unwrap();
        let answer = tokio::time::timeout(std::time::Duration::from_secs(20), connection.recv());
        let answer = answer.await.expect("the relay never answered").unwrap();
        assert_eq!(answer[..8], [0; 8]);
        assert_eq!(answer[20..24], types::ResPQ::CONSTRUCTOR_ID.to_le_bytes());
        assert_eq!(answer[24..40], nonce);
    }

    #[test]
    fn rfc6455_accept_key_sample() {
        assert_eq!(accept_key("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[tokio::test]
    async fn frames_round_trip_125_126_65535_65536() {
        let ((mut reader, mut writer), mut server) = pair();
        // An empty write sends nothing at all, so the sizes start at 1.
        for len in [1, 125, 126, 65535, 65536] {
            let payload: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
            writer.write_all(&payload).await.unwrap();
            writer.flush().await.unwrap();
            assert_eq!(client_frame(&mut server).await, (BINARY, payload.clone()), "{len}");
            // A packet goes out from its own buffer, its header written into the room in front.
            let mut buf = [vec![0xff; ROOM], payload.clone()].concat();
            writer.send_in_place(&mut buf, ROOM).await.unwrap();
            assert_eq!(client_frame(&mut server).await, (BINARY, payload.clone()), "{len} in place");
            server.write_all(&server_frame(true, BINARY, &payload)).await.unwrap();
            let mut back = vec![0; len];
            reader.read_exact(&mut back).await.unwrap();
            assert_eq!(back, payload, "{len}");
        }
    }

    #[tokio::test]
    async fn reader_handles_ping_continuation_close() {
        let ((mut reader, mut writer), mut server) = pair();
        let frames = [
            server_frame(false, BINARY, b"ab"),
            server_frame(true, PING, b"hi"),
            server_frame(true, CONTINUATION, b"cd"),
            server_frame(true, BINARY, b""),
            server_frame(true, PONG, b"ignored"),
            server_frame(true, CLOSE, &1000u16.to_be_bytes()),
        ];
        server.write_all(&frames.concat()).await.unwrap();
        let mut got = Vec::new();
        reader.read_to_end(&mut got).await.unwrap();
        assert_eq!(got, b"abcd");
        // The ping is answered before the next frame.
        writer.write_all(b"x").await.unwrap();
        writer.flush().await.unwrap();
        assert_eq!(client_frame(&mut server).await, (PONG, b"hi".to_vec()));
        assert_eq!(client_frame(&mut server).await, (BINARY, b"x".to_vec()));
    }

    #[tokio::test]
    async fn two_pings_before_a_write_get_two_pongs() {
        let ((mut reader, mut writer), mut server) = pair();
        let frames =
            [server_frame(true, PING, b"one"), server_frame(true, PING, b"two"), server_frame(true, BINARY, b"x")];
        server.write_all(&frames.concat()).await.unwrap();
        reader.read_exact(&mut [0; 1]).await.unwrap();
        writer.write_all(b"y").await.unwrap();
        writer.flush().await.unwrap();
        assert_eq!(client_frame(&mut server).await, (PONG, b"one".to_vec()));
        assert_eq!(client_frame(&mut server).await, (PONG, b"two".to_vec()));
        assert_eq!(client_frame(&mut server).await, (BINARY, b"y".to_vec()));
    }

    #[tokio::test]
    async fn a_ping_flood_keeps_only_the_latest_pongs() {
        let ((mut reader, mut writer), mut server) = pair();
        let pings: Vec<u8> = (0..20u8).flat_map(|i| server_frame(true, PING, &[i])).collect();
        server.write_all(&[pings, server_frame(true, BINARY, b"x")].concat()).await.unwrap();
        reader.read_exact(&mut [0; 1]).await.unwrap();
        // Twenty pings, one wake, and only the last MAX_PONGS answers kept.
        writer.control_ready().await;
        writer.flush_control().await.unwrap();
        for i in 20 - MAX_PONGS as u8..20 {
            assert_eq!(client_frame(&mut server).await, (PONG, vec![i]));
        }
        writer.write_all(b"y").await.unwrap();
        writer.flush().await.unwrap();
        assert_eq!(client_frame(&mut server).await, (BINARY, b"y".to_vec()));
    }

    #[tokio::test]
    async fn an_idle_connection_answers_a_ping_without_an_outgoing_message() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let at = listener.local_addr().unwrap();
        let mut server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            answer_upgrade(&mut stream, None, b"").await;
            // The obfuscation header, and after it nothing from the client but the pong.
            assert_eq!(client_frame(&mut stream).await.0, BINARY);
            stream.write_all(&server_frame(true, PING, b"idle")).await.unwrap();
            let pong = tokio::time::timeout(std::time::Duration::from_secs(1), client_frame(&mut stream)).await;
            // The stream is kept open, so the client's read doesn't end first.
            (pong.ok(), stream)
        });
        let dc = Dc { id: 2, addr: at, test: false };
        let connection = Connection::open_websocket(&dc, &Options::default(), &Endpoint::plain(at)).await.unwrap();
        let (mut reader, mut writer) = connection.into_split();
        // As `Runner::run` does: read, and send control frames whenever they're ready.
        let (pong, _stream) = loop {
            tokio::select! {
                answered = &mut server => break answered.unwrap(),
                packet = reader.recv() => panic!("the server sent no packet: {packet:?}"),
                () = writer.control_ready() => writer.flush_control().await.unwrap(),
            }
        };
        assert_eq!(pong, Some((PONG, b"idle".to_vec())), "no pong within 1 s");
    }

    #[tokio::test]
    async fn websocket_through_socks5_sends_the_hostname() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let at = listener.local_addr().unwrap();
        let options = Options {
            proxy: Some(Proxy::Socks5 { host: at.ip().to_string(), port: at.port(), auth: None }),
            ..Options::default()
        };
        // `.invalid` never resolves (RFC 6761): only the proxy can take this name anywhere.
        let endpoint = Endpoint { host: "venus.invalid".into(), port: 443, path: "/apiws", tls: false };
        let request = [&[5, 1, 0, 3, 13][..], b"venus.invalid", &443u16.to_be_bytes()].concat();
        // The proxy takes the first request and refuses the second with reply 4, host unreachable.
        let proxy = tokio::spawn(async move {
            for reply in [0, 4] {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut greeting = [0; 3];
                stream.read_exact(&mut greeting).await.unwrap();
                assert_eq!(greeting, [5, 1, 0]);
                stream.write_all(&[5, 0]).await.unwrap();
                let mut got = vec![0; request.len()];
                stream.read_exact(&mut got).await.unwrap();
                assert_eq!(got, request);
                stream.write_all(&[5, reply, 0, 1, 0, 0, 0, 0, 0, 0]).await.unwrap();
            }
        });
        dial(&endpoint, &options).await.unwrap();
        // A refused name fails the connect; nothing falls back to a lookup on this side.
        let refused = dial(&endpoint, &options).await.err().map(|e| e.to_string());
        assert!(refused.as_deref().is_some_and(|e| e.contains("venus.invalid:443 (reply 4)")), "{refused:?}");
        proxy.await.unwrap();
    }

    #[tokio::test]
    async fn rejects_masked_server_and_text_frames() {
        let masked = [0x82, 0x81, 1, 2, 3, 4, b'a' ^ 1].to_vec();
        let reserved = [0xc2, 0x01, b'a'].to_vec();
        let too_long = [&[0x82, 127][..], &(17u64 << 20).to_be_bytes()].concat();
        let long_ping = [&[0x89, 126][..], &200u16.to_be_bytes()].concat();
        let fragmented_ping = server_frame(false, PING, b"hi");
        for frame in [masked, server_frame(true, TEXT, b"hi"), reserved, too_long, long_ping, fragmented_ping] {
            let ((mut reader, _writer), mut server) = pair();
            server.write_all(&frame).await.unwrap();
            let kind = reader.read(&mut [0; 16]).await.err().map(|e| e.kind());
            assert_eq!(kind, Some(std::io::ErrorKind::InvalidData), "{frame:?}");
        }
    }
}
