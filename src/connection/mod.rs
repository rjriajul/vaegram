#[cfg(feature = "dns-config")]
mod dns_config;
mod fake_tls;
#[cfg(any(feature = "websocket", feature = "dns-config", feature = "web-proxy"))]
mod http1;
mod obfuscated;
mod proxy;
#[cfg(any(feature = "websocket", feature = "dns-config", feature = "web-proxy"))]
mod tls;
mod transport;
#[cfg(feature = "web-proxy")]
mod web_proxy;
#[cfg(feature = "websocket")]
mod websocket;

#[cfg(feature = "dns-config")]
pub use dns_config::{AccessPointRule, DnsConfig, IpPort};
pub use proxy::{Proxy, Secret};
pub use transport::Transport;
pub(crate) use transport::{Framing, RecvBuf};

use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

use crate::errors::{Error, Result};
use crate::session::internals::data_center::Dc;
use crate::session::{Call, Observer, Retry};
use crate::storage::recording::Recorder;
use crate::types::Json;
use fake_tls::{RecordReader, RecordWriter};
use obfuscated::Ctr;

const READ_CHUNK: usize = 64 << 10;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
// A web proxy's setup is several HTTPS requests and a long poll for WELCOME, each with its own deadline; this only
// bounds the whole.
#[cfg(feature = "web-proxy")]
const WEB_PROXY_CONNECT_TIMEOUT: Duration = Duration::from_secs(90);

/// A byte stream a [`ConnectionFactory`] hands back: a TCP socket, a TLS session, a tunnel, anything.
pub trait Stream: AsyncRead + AsyncWrite + Send + Unpin {}

impl<T: AsyncRead + AsyncWrite + Send + Unpin> Stream for T {}

/// The future a [`ConnectionFactory`] returns: the opened stream, or why it couldn't be opened.
pub type Connecting<'a> = Pin<Box<dyn Future<Output = io::Result<Box<dyn Stream>>> + Send + 'a>>;

/// Opens the byte stream a connection runs over, in place of a plain TCP connect: to bind a local address, enter a
/// network namespace or tunnel. It's given the data center's address, or the proxy's when one is set (the proxy
/// handshake then runs over the stream). A closure `Fn(SocketAddr) -> impl Future<Output = io::Result<impl Stream>>`
/// is one. Transfer and CDN connections use it too.
pub trait ConnectionFactory: Send + Sync {
    /// Opens a stream to `addr`. An error fails that connection attempt like a refused TCP connect.
    fn connect(&self, addr: SocketAddr) -> Connecting<'_>;
}

impl<F, Fut, S> ConnectionFactory for F
where
    F: Fn(SocketAddr) -> Fut + Send + Sync,
    Fut: Future<Output = io::Result<S>> + Send + 'static,
    S: Stream + 'static,
{
    fn connect(&self, addr: SocketAddr) -> Connecting<'_> {
        let stream = self(addr);
        Box::pin(async move { Ok(Box::new(stream.await?) as Box<dyn Stream>) })
    }
}

/// Lets a factory that is already shared be passed in.
impl<T: ConnectionFactory + ?Sized> ConnectionFactory for Arc<T> {
    fn connect(&self, addr: SocketAddr) -> Connecting<'_> {
        (**self).connect(addr)
    }
}

impl std::fmt::Debug for dyn ConnectionFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ConnectionFactory")
    }
}

/// How a session connects and talks to Telegram: transport, proxy, timeouts, retries and the device it reports.
/// [`ClientBuilder`](crate::ClientBuilder) fills one from its settings. Only the `unstable-raw` plumbing
/// (`Connection::connect` and `Session::connect`) takes an `Options`, so without that feature one built with the
/// setters here has no use.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Options {
    /// The framing of packets on the wire; [`Transport::Intermediate`] by default.
    pub transport: Transport,
    /// Hides the transport from DPI with an obfuscation header; off by default. MTProxy connections are always
    /// obfuscated, and the full transport can't be.
    pub obfuscated: bool,
    /// The proxy every connection goes through; `None` connects directly.
    pub proxy: Option<Proxy>,
    /// Opens every connection's stream instead of a TCP connect; see [`ConnectionFactory`].
    pub connection_factory: Option<Arc<dyn ConnectionFactory>>,
    /// How long a request waits for its answer before it fails with `Error::Timeout`; 30 seconds by default.
    pub request_timeout: Duration,
    /// Asked after every failed request whether to send it again, and after how long. The default sleeps off
    /// FLOOD_WAITs of up to a minute, five times per request.
    pub retry: Retry,
    /// Every request is sent inside `invokeWithoutUpdates`, so the connection never gets updates. Set by
    /// `ClientBuilder::no_updates`.
    pub no_updates: bool,
    /// Update batches held for the dispatcher. When it's full, a batch is dropped and a getDifference brings
    /// back what it held.
    pub updates_capacity: usize,
    /// Sees every request that ends; `None` by default.
    pub observer: Option<Observer>,
    /// Writes every message sent and received to a recording; `None` by default.
    pub record: Option<Recorder>,
    /// What the connection tells Telegram about the client in `initConnection`.
    pub device: Device,
    /// Carries MTProto over WebSocket on TLS (`wss://…web.telegram.org/apiws`), always obfuscated, instead of a
    /// direct TCP connection: for networks that only let HTTPS through. Set by `ClientBuilder::websocket`.
    #[cfg(feature = "websocket")]
    pub websocket: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            transport: Transport::default(),
            obfuscated: false,
            proxy: None,
            connection_factory: None,
            request_timeout: Duration::from_secs(30),
            retry: Retry::default(),
            no_updates: false,
            updates_capacity: 1024,
            observer: None,
            record: None,
            device: Device::default(),
            #[cfg(feature = "websocket")]
            websocket: false,
        }
    }
}

impl Options {
    /// Same as `Default`: the intermediate transport over a direct, unobfuscated TCP connection, a 30 s request
    /// timeout, room for 1024 update batches and the default retry policy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Picks the MTProto transport the connection frames packets with.
    #[must_use]
    pub fn transport(mut self, transport: Transport) -> Self {
        self.transport = transport;
        self
    }

    /// Sets whether the transport is obfuscated. The full transport refuses obfuscation when connecting.
    #[must_use]
    pub fn obfuscated(mut self, obfuscated: bool) -> Self {
        self.obfuscated = obfuscated;
        self
    }

    /// Connects through `proxy` instead of directly.
    #[must_use]
    pub fn proxy(mut self, proxy: Proxy) -> Self {
        self.proxy = Some(proxy);
        self
    }

    /// Opens every connection's stream with `factory` instead of a TCP connect; see [`ConnectionFactory`].
    #[must_use]
    pub fn connection_factory(mut self, factory: impl ConnectionFactory + 'static) -> Self {
        self.connection_factory = Some(Arc::new(factory));
        self
    }

    /// Sets how long a request may wait for its answer.
    #[must_use]
    pub fn request_timeout(mut self, request_timeout: Duration) -> Self {
        self.request_timeout = request_timeout;
        self
    }

    /// Decides whether and when a failed request is sent again.
    #[must_use]
    pub fn retry(mut self, retry: Retry) -> Self {
        self.retry = retry;
        self
    }

    /// Sets whether every request goes inside `invokeWithoutUpdates`, so the connection never gets updates.
    #[must_use]
    pub fn no_updates(mut self, no_updates: bool) -> Self {
        self.no_updates = no_updates;
        self
    }

    /// Sets how many update batches are held for the dispatcher before one is dropped.
    #[must_use]
    pub fn updates_capacity(mut self, updates_capacity: usize) -> Self {
        self.updates_capacity = updates_capacity;
        self
    }

    /// Has `observer` see every request. It runs on the connection's task, so it should return quickly.
    #[must_use]
    pub fn observer(mut self, observer: impl Fn(&Call<'_>) + Send + Sync + 'static) -> Self {
        self.observer = Some(Observer(Arc::new(observer)));
        self
    }

    /// Records the traffic into `record`.
    #[must_use]
    pub fn record(mut self, record: Recorder) -> Self {
        self.record = Some(record);
        self
    }

    /// Sets the device the connection describes to Telegram in `initConnection`.
    #[must_use]
    pub fn device(mut self, device: Device) -> Self {
        self.device = device;
        self
    }

    /// Sets whether MTProto goes over WebSocket on TLS instead of a direct TCP connection.
    #[cfg(feature = "websocket")]
    #[must_use]
    pub fn websocket(mut self, websocket: bool) -> Self {
        self.websocket = websocket;
        self
    }

    #[cfg(feature = "websocket")]
    pub(crate) fn uses_websocket(&self) -> bool {
        self.websocket
    }

    #[cfg(not(feature = "websocket"))]
    pub(crate) fn uses_websocket(&self) -> bool {
        false
    }
}

/// The client details sent in `initConnection`, which Telegram shows in the account's list of active sessions.
/// The default is model `vaegram`, the OS name as the system version, this crate's version as the app version and
/// language `en`.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct Device {
    /// The device model, `"vaegram"` by default.
    pub model: String,
    /// The operating system version, the OS name (`std::env::consts::OS`) by default.
    pub system_version: String,
    /// The application version, this crate's version by default.
    pub app_version: String,
    /// The language code of the user interface, such as `en`; `en` by default.
    pub lang_code: String,
    /// The official language pack the client uses (`android`, `ios`, `tdesktop`, …); empty by default.
    pub lang_pack: String,
    /// The device's own language; `None` sends `lang_code`.
    pub system_lang_code: Option<String>,
    /// The client's UTC offset in seconds, sent as `tz_offset` in `initConnection.params`; `None` sends none.
    pub tz_offset: Option<i32>,
    /// The JSON `initConnection.params` carries; `None` sends none. With a `tz_offset` set too, it must be an
    /// object: its `tz_offset` key is replaced, or added if it has none.
    pub params: Option<Json>,
}

impl Default for Device {
    fn default() -> Self {
        Device {
            model: "vaegram".into(),
            system_version: std::env::consts::OS.into(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            lang_code: "en".into(),
            lang_pack: String::new(),
            system_lang_code: None,
            tz_offset: None,
            params: None,
        }
    }
}

unstable_raw_pub! {
    /// One transport connection to a data center, framed and obfuscated as its [`Options`] say. It carries MTProto
    /// packets without MTProto encryption, which the [`Session`](crate::session::Session) above it adds; each session
    /// opens its own connections rather than taking one.
    ///
    /// Experimental (`unstable-raw`): may change or go away in any release.
    struct Connection {
        reader: Reader,
        writer: Writer,
    }
}

impl Connection {
    /// Opens the stream, through the proxy or connection factory when set, and sends the transport's opening
    /// bytes. Fails with `Error::InvalidArgument` for the full transport with obfuscation or an MTProxy, or for
    /// WebSocket with an MTProxy or the full transport, and with an I/O `TimedOut` error after 10 seconds (90
    /// through a web proxy).
    pub async fn connect(dc: &Dc, options: &Options) -> Result<Self> {
        let addr = dc.addr;
        let via = if options.proxy.is_some() { " through the proxy" } else { "" };
        #[cfg(feature = "web-proxy")]
        let limit = match options.proxy {
            Some(Proxy::Web { .. }) => WEB_PROXY_CONNECT_TIMEOUT,
            _ => CONNECT_TIMEOUT,
        };
        #[cfg(not(feature = "web-proxy"))]
        let limit = CONNECT_TIMEOUT;
        tokio::time::timeout(limit, Self::open(dc, options))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, format!("connecting to {addr}{via} timed out")))?
    }

    async fn open(dc: &Dc, options: &Options) -> Result<Self> {
        let addr = dc.addr;
        let secret = options.proxy.as_ref().and_then(Proxy::secret);
        // The full transport sends no header to hide, so obfuscation has nothing to wrap, and an MTProxy only
        // understands the obfuscated ones.
        if options.transport == Transport::Full && (options.obfuscated || secret.is_some()) {
            return Err(Error::InvalidArgument("the full transport can't be obfuscated or go through an MTProxy"));
        }
        #[cfg(feature = "websocket")]
        if options.websocket {
            if secret.is_some() || options.transport == Transport::Full {
                return Err(Error::InvalidArgument("WebSocket can't go through an MTProxy or use the full transport"));
            }
            return Self::open_websocket(dc, options, &websocket::Endpoint::of(dc)?).await;
        }
        let (read, write) = match (&options.connection_factory, &options.proxy) {
            #[cfg(feature = "web-proxy")]
            (factory, Some(Proxy::Web { host, secret })) => {
                let stream: Box<dyn Stream> = Box::new(web_proxy::open(host, secret, factory.clone()).await?);
                let (read, write) = tokio::io::split(stream);
                (ReadHalf::Boxed(read), WriteHalf::Boxed(write))
            }
            (None, proxy) => {
                let stream = match proxy {
                    None => TcpStream::connect(addr).await?,
                    Some(proxy) => {
                        let mut stream = TcpStream::connect(proxy.endpoint()).await?;
                        proxy.handshake(&mut stream, addr).await?;
                        stream
                    }
                };
                stream.set_nodelay(true)?;
                let (read, write) = stream.into_split();
                (ReadHalf::Tcp(read), WriteHalf::Tcp(write))
            }
            (Some(factory), proxy) => {
                let mut stream = match proxy {
                    None => factory.connect(addr).await?,
                    Some(proxy) => {
                        let mut stream = dial_any(factory.as_ref(), proxy.endpoint()).await?;
                        proxy.handshake(&mut stream, addr).await?;
                        stream
                    }
                };
                stream.flush().await?;
                let (read, write) = tokio::io::split(stream);
                (ReadHalf::Boxed(read), WriteHalf::Boxed(write))
            }
        };
        Self::start(dc, options, read, write, options.obfuscated || secret.is_some()).await
    }

    #[cfg(feature = "websocket")]
    async fn open_websocket(dc: &Dc, options: &Options, endpoint: &websocket::Endpoint) -> Result<Self> {
        // The data center's address family is the one asked for, so a proxy is given that family first too.
        let (read, write) = websocket::open(endpoint, options, dc.addr.is_ipv6()).await?;
        // Telegram's WebSocket endpoints only take obfuscated streams.
        let read = ReadHalf::WebSocket(Box::new(read));
        Self::start(dc, options, read, WriteHalf::WebSocket(write), true).await
    }

    /// Sends what the transport starts with, the obfuscation header or the transport's tag, on a stream just opened.
    async fn start(dc: &Dc, options: &Options, read: ReadHalf, mut write: WriteHalf, obfuscate: bool) -> Result<Self> {
        let secret = options.proxy.as_ref().and_then(Proxy::secret);
        let transport =
            if secret.is_some_and(|s| s.padded) { Transport::PaddedIntermediate } else { options.transport };
        let tls = secret.is_some_and(|s| s.domain.is_some());
        let (encrypt, decrypt, records) = match transport.tag() {
            Some(tag) if obfuscate => {
                let (header, encrypt, decrypt) = obfuscated::init(tag, dc.wire_id() as i16, secret.map(|s| &s.key));
                // Behind fake TLS the header rides in the first record together with the first packet.
                let records = tls.then(|| RecordWriter::new(header));
                if !tls {
                    write.write_all(&header).await?;
                }
                (Some(encrypt), Some(decrypt), records)
            }
            Some(tag) => {
                write.write_all(if transport == Transport::Abridged { &tag[..1] } else { &tag[..] }).await?;
                (None, None, None)
            }
            // The full transport starts with its first packet.
            None => (None, None, None),
        };
        let framing = Framing::new(transport);
        Ok(Self {
            reader: Reader {
                wire: Wire { half: read, records: tls.then(RecordReader::default) },
                framing,
                decrypt,
                buf: RecvBuf::default(),
            },
            writer: Writer { room: write.room(), half: write, framing, encrypt, records, buf: Vec::new() },
        })
    }

    /// Frames `payload` and writes it out.
    pub async fn send(&mut self, payload: &[u8]) -> Result<()> {
        self.writer.send(payload).await
    }

    /// Waits for the next whole packet and returns its payload. Fails with `UnexpectedEof` once the other side
    /// closes the stream.
    pub async fn recv(&mut self) -> Result<Vec<u8>> {
        self.reader.recv().await
    }

    pub(crate) fn into_split(self) -> (Reader, Writer) {
        (self.reader, self.writer)
    }
}

/// Dials the first address of `host` the factory can open, as a TCP connect to a host name tries each in turn.
async fn dial_any(factory: &dyn ConnectionFactory, host: (&str, u16)) -> io::Result<Box<dyn Stream>> {
    let mut last = io::Error::other("the host has no address");
    for addr in tokio::net::lookup_host(host).await? {
        match factory.connect(addr).await {
            Ok(stream) => return Ok(stream),
            Err(e) => last = e,
        }
    }
    Err(last)
}

pub(crate) struct Reader {
    wire: Wire,
    framing: Framing,
    decrypt: Option<Ctr>,
    buf: RecvBuf,
}

impl Reader {
    /// Cancel-safe: bytes wait in the buffer until a whole packet is there.
    pub(crate) async fn recv(&mut self) -> Result<Vec<u8>> {
        loop {
            if let Some(packet) = self.framing.decode(&mut self.buf)? {
                return Ok(packet);
            }
            let buf = self.buf.make_room();
            buf.reserve(READ_CHUNK);
            let start = buf.len();
            if self.wire.read_buf(buf).await? == 0 {
                return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
            }
            if let Some(decrypt) = &mut self.decrypt {
                decrypt.apply(&mut buf[start..]);
            }
        }
    }
}

struct Wire {
    half: ReadHalf,
    records: Option<RecordReader>,
}

impl AsyncRead for Wire {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let this = &mut *self;
        match &mut this.records {
            Some(records) => records.poll_read(&mut this.half, cx, buf),
            None => Pin::new(&mut this.half).poll_read(cx, buf),
        }
    }
}

// TCP keeps its own halves, so the default path has no boxing or dynamic dispatch.
enum ReadHalf {
    Tcp(OwnedReadHalf),
    Boxed(tokio::io::ReadHalf<Box<dyn Stream>>),
    #[cfg(feature = "websocket")]
    WebSocket(Box<websocket::Read>),
}

impl AsyncRead for ReadHalf {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            ReadHalf::Tcp(half) => Pin::new(half).poll_read(cx, buf),
            ReadHalf::Boxed(half) => Pin::new(half).poll_read(cx, buf),
            #[cfg(feature = "websocket")]
            ReadHalf::WebSocket(half) => Pin::new(half).poll_read(cx, buf),
        }
    }
}

enum WriteHalf {
    Tcp(OwnedWriteHalf),
    Boxed(tokio::io::WriteHalf<Box<dyn Stream>>),
    #[cfg(feature = "websocket")]
    WebSocket(websocket::Write),
}

impl WriteHalf {
    /// Free bytes the layer below needs in front of a framed packet.
    fn room(&self) -> usize {
        match self {
            #[cfg(feature = "websocket")]
            WriteHalf::WebSocket(_) => websocket::ROOM,
            _ => 0,
        }
    }
}

impl AsyncWrite for WriteHalf {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            WriteHalf::Tcp(half) => Pin::new(half).poll_write(cx, buf),
            WriteHalf::Boxed(half) => Pin::new(half).poll_write(cx, buf),
            #[cfg(feature = "websocket")]
            WriteHalf::WebSocket(half) => Pin::new(half).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            WriteHalf::Tcp(half) => Pin::new(half).poll_flush(cx),
            WriteHalf::Boxed(half) => Pin::new(half).poll_flush(cx),
            #[cfg(feature = "websocket")]
            WriteHalf::WebSocket(half) => Pin::new(half).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            WriteHalf::Tcp(half) => Pin::new(half).poll_shutdown(cx),
            WriteHalf::Boxed(half) => Pin::new(half).poll_shutdown(cx),
            #[cfg(feature = "websocket")]
            WriteHalf::WebSocket(half) => Pin::new(half).poll_shutdown(cx),
        }
    }
}

pub(crate) struct Writer {
    half: WriteHalf,
    // Bytes kept free in front of the framing room, for the WebSocket frame header.
    room: usize,
    framing: Framing,
    encrypt: Option<Ctr>,
    records: Option<RecordWriter>,
    pub(crate) buf: Vec<u8>,
}

impl Writer {
    pub(crate) fn begin(&mut self) {
        self.buf.clear();
        self.buf.resize(self.room + Framing::ROOM, 0);
    }

    pub(crate) async fn finish(&mut self) -> Result<()> {
        let start = self.framing.frame_after(&mut self.buf, self.room);
        if let Some(encrypt) = &mut self.encrypt {
            encrypt.apply(&mut self.buf[start..]);
        }
        #[cfg(feature = "websocket")]
        if let WriteHalf::WebSocket(half) = &mut self.half {
            half.send_in_place(&mut self.buf, start).await?;
            return Ok(());
        }
        let wire = match &mut self.records {
            Some(records) => records.wrap(&self.buf[start..]),
            None => &self.buf[start..],
        };
        self.half.write_all(wire).await?;
        // A boxed stream may buffer (TLS does); TCP's flush is a no-op.
        self.half.flush().await?;
        Ok(())
    }

    pub(crate) async fn send(&mut self, payload: &[u8]) -> Result<()> {
        self.begin();
        self.buf.extend_from_slice(payload);
        self.finish().await
    }
}

#[cfg(test)]
mod tests {
    use tokio::net::TcpListener;

    use super::*;

    fn full() -> Options {
        Options { transport: Transport::Full, ..Options::default() }
    }

    #[cfg(feature = "websocket")]
    #[tokio::test]
    async fn websocket_refuses_mtproxy_and_full() {
        // A data center with no WebSocket name: getting past the refusal would fail differently.
        let dc = Dc { id: 9, addr: "127.0.0.1:9".parse().unwrap(), test: false };
        let secret = "00".repeat(16).parse().unwrap();
        let mtproxy = Proxy::MtProxy { host: "127.0.0.1".into(), port: 9, secret };
        let websocket = Options { websocket: true, ..Options::default() };
        let refused = [
            Options { proxy: Some(mtproxy), ..websocket.clone() },
            Options { transport: Transport::Full, ..websocket.clone() },
        ];
        for options in refused {
            let result = Connection::connect(&dc, &options).await;
            assert!(matches!(result, Err(Error::InvalidArgument(_))), "{options:?}");
        }
        assert!(matches!(Connection::connect(&dc, &websocket).await, Err(Error::InvalidDc(9))));
    }

    #[tokio::test]
    async fn full_with_obfuscation_is_refused() {
        // Nothing listens here: the refusal has to come before any connection is tried.
        let dc = Dc { id: 2, addr: "127.0.0.1:9".parse().unwrap(), test: false };
        let secret = "00".repeat(16).parse().unwrap();
        let mtproxy = Proxy::MtProxy { host: "127.0.0.1".into(), port: 9, secret };
        for options in [Options { obfuscated: true, ..full() }, Options { proxy: Some(mtproxy), ..full() }] {
            let refused = Connection::connect(&dc, &options).await;
            assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{options:?}");
        }
    }

    #[tokio::test]
    async fn full_sends_no_preamble_and_numbers_its_packets() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let dc = Dc { id: 2, addr: listener.local_addr().unwrap(), test: false };
        // A server that reads whole full packets from the very first byte and sends each one back.
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let (mut incoming, mut outgoing) = (Framing::new(Transport::Full), Framing::new(Transport::Full));
            let mut buf = RecvBuf::default();
            let mut got = Vec::new();
            while got.len() < 2 {
                if let Some(packet) = incoming.decode(&mut buf).unwrap() {
                    got.push(packet);
                } else {
                    assert!(stream.read_buf(buf.make_room()).await.unwrap() > 0);
                }
            }
            for packet in &got {
                let mut frame = vec![0; Framing::ROOM];
                frame.extend_from_slice(packet);
                let start = outgoing.frame(&mut frame);
                stream.write_all(&frame[start..]).await.unwrap();
            }
            got
        });
        let mut connection = Connection::connect(&dc, &full()).await.unwrap();
        connection.send(b"abcd1234").await.unwrap();
        connection.send(b"efgh5678").await.unwrap();
        assert_eq!(connection.recv().await.unwrap(), b"abcd1234");
        assert_eq!(connection.recv().await.unwrap(), b"efgh5678");
        assert_eq!(server.await.unwrap(), [b"abcd1234".to_vec(), b"efgh5678".to_vec()]);
    }

    /// Reads the intermediate tag and one packet, then sends the packet back.
    async fn echo_one(stream: &mut (impl AsyncRead + AsyncWrite + Unpin)) -> [u8; 4] {
        let (mut tag, mut len) = ([0; 4], [0; 4]);
        stream.read_exact(&mut tag).await.unwrap();
        stream.read_exact(&mut len).await.unwrap();
        let mut payload = vec![0; u32::from_le_bytes(len) as usize];
        stream.read_exact(&mut payload).await.unwrap();
        stream.write_all(&len).await.unwrap();
        stream.write_all(&payload).await.unwrap();
        tag
    }

    /// A factory that hands out one end of an in-memory pipe, never a socket, and records what it was asked for.
    fn piped() -> (Options, tokio::io::DuplexStream, Arc<std::sync::Mutex<Vec<SocketAddr>>>) {
        let (ours, theirs) = tokio::io::duplex(1 << 16);
        let (dialed, end) = (Arc::new(std::sync::Mutex::new(Vec::new())), std::sync::Mutex::new(Some(ours)));
        let seen = dialed.clone();
        let factory = move |addr: SocketAddr| {
            seen.lock().unwrap().push(addr);
            let end = end.lock().unwrap().take();
            async move { end.ok_or_else(|| io::Error::other("dialed twice")) }
        };
        (Options { connection_factory: Some(Arc::new(factory)), ..Options::default() }, theirs, dialed)
    }

    #[tokio::test]
    async fn custom_connector_carries_the_handshake() {
        let dc = Dc { id: 2, addr: "149.154.167.51:443".parse().unwrap(), test: false };
        let (options, mut server, dialed) = piped();
        let server = tokio::spawn(async move { echo_one(&mut server).await });
        let mut connection = Connection::connect(&dc, &options).await.unwrap();
        connection.send(b"abcd1234").await.unwrap();
        assert_eq!(connection.recv().await.unwrap(), b"abcd1234");
        assert_eq!((server.await.unwrap(), dialed.lock().unwrap().clone()), ([0xee; 4], vec![dc.addr]));
    }

    #[tokio::test]
    async fn proxy_runs_over_connector() {
        let dc = Dc { id: 2, addr: "149.154.167.51:443".parse().unwrap(), test: false };
        let (options, mut server, dialed) = piped();
        let proxy = Proxy::Http { host: "127.0.0.1".into(), port: 8080, auth: None };
        let options = Options { proxy: Some(proxy), ..options };
        let server = tokio::spawn(async move {
            let mut head = Vec::new();
            while !head.ends_with(b"\r\n\r\n") {
                head.push(server.read_u8().await.unwrap());
            }
            server.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await.unwrap();
            (head, echo_one(&mut server).await)
        });
        let mut connection = Connection::connect(&dc, &options).await.unwrap();
        connection.send(b"abcd1234").await.unwrap();
        assert_eq!(connection.recv().await.unwrap(), b"abcd1234");
        let (head, tag) = server.await.unwrap();
        assert!(head.starts_with(b"CONNECT 149.154.167.51:443 "), "{}", String::from_utf8_lossy(&head));
        // The factory is asked for the proxy; the data center is reached through it.
        assert_eq!((tag, dialed.lock().unwrap().clone()), ([0xee; 4], vec!["127.0.0.1:8080".parse().unwrap()]));
    }

    #[tokio::test]
    async fn every_proxy_address_is_tried() {
        let dc = Dc { id: 2, addr: "149.154.167.51:443".parse().unwrap(), test: false };
        let dialed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = dialed.clone();
        // "localhost" may give an IPv6 address first; only an IPv4 one opens, onto a pipe nobody reads.
        let factory = move |addr: SocketAddr| {
            seen.lock().unwrap().push(addr);
            async move {
                match addr.is_ipv4() {
                    true => Ok(tokio::io::duplex(64).0),
                    false => Err(io::Error::new(io::ErrorKind::ConnectionRefused, "no IPv6 here")),
                }
            }
        };
        let proxy = Proxy::Http { host: "localhost".into(), port: 8080, auth: None };
        let options = Options { proxy: Some(proxy), connection_factory: Some(Arc::new(factory)), ..Options::default() };
        assert!(Connection::connect(&dc, &options).await.is_err());
        // Dialing stopped at the first address that opened, however the host resolved.
        assert!(dialed.lock().unwrap().last().is_some_and(SocketAddr::is_ipv4), "{:?}", dialed.lock().unwrap());
    }

    #[tokio::test]
    async fn a_connector_error_fails_connect() {
        let dc = Dc { id: 2, addr: "149.154.167.51:443".parse().unwrap(), test: false };
        let refuse = |_: SocketAddr| async {
            Err::<tokio::io::DuplexStream, _>(io::Error::new(io::ErrorKind::ConnectionRefused, "no route"))
        };
        let options = Options { connection_factory: Some(Arc::new(refuse)), ..Options::default() };
        let result = Connection::connect(&dc, &options).await;
        assert!(
            matches!(&result, Err(Error::Io(e)) if e.kind() == io::ErrorKind::ConnectionRefused),
            "{:?}",
            result.err()
        );
        assert_eq!(format!("{:?}", options.connection_factory.unwrap()), "ConnectionFactory");
    }

    #[tokio::test]
    async fn options_build_from_new_and_setters() {
        let options = Options::new();
        assert_eq!(
            (options.transport, options.obfuscated, options.updates_capacity),
            (Transport::Intermediate, false, 1024)
        );
        assert_eq!(options.request_timeout, Duration::from_secs(30));
        assert!(options.proxy.is_none() && options.connection_factory.is_none() && !options.no_updates);
        let dc = Dc { id: 2, addr: "149.154.167.51:443".parse().unwrap(), test: false };
        let refuse = |_: SocketAddr| async {
            Err::<tokio::io::DuplexStream, _>(io::Error::new(io::ErrorKind::ConnectionRefused, "no route"))
        };
        // An already shared factory can be passed in.
        let shared: Arc<dyn ConnectionFactory> = Arc::new(refuse);
        let result = Connection::connect(&dc, &Options::new().connection_factory(Arc::clone(&shared))).await;
        assert!(matches!(&result, Err(Error::Io(e)) if e.kind() == io::ErrorKind::ConnectionRefused));
        // The full transport refuses obfuscation before anything is dialed.
        let full = Options::new().transport(Transport::Full).obfuscated(true).connection_factory(refuse);
        assert!(matches!(Connection::connect(&dc, &full).await, Err(Error::InvalidArgument(_))));
    }
}
