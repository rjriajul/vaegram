mod fake_tls;
mod obfuscated;
mod proxy;
mod transport;

pub use proxy::{Proxy, Secret};
pub use transport::{RecvBuf, Transport};

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

use crate::errors::Result;
use crate::session::Observer;
use crate::session::internals::data_center::Dc;
use crate::storage::recording::Recorder;
use fake_tls::{RecordReader, RecordWriter};
use obfuscated::Ctr;

const READ_CHUNK: usize = 64 << 10;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug)]
pub struct Options {
    pub transport: Transport,
    pub obfuscated: bool,
    pub proxy: Option<Proxy>,
    pub request_timeout: Duration,
    /// The longest FLOOD_WAIT `Client::invoke` and file transfers sleep off and retry; zero hands every one to
    /// the caller.
    pub flood_sleep_threshold: Duration,
    /// How many FLOOD_WAITs one call sleeps off; the next goes to the caller.
    pub max_flood_sleeps: usize,
    /// Update batches held for the dispatcher. When it's full, a batch is dropped and a getDifference brings
    /// back what it held.
    pub updates_capacity: usize,
    pub observer: Option<Observer>,
    pub record: Option<Recorder>,
    pub device: Device,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            transport: Transport::default(),
            obfuscated: false,
            proxy: None,
            request_timeout: Duration::from_secs(30),
            flood_sleep_threshold: Duration::from_secs(60),
            max_flood_sleeps: 5,
            updates_capacity: 1024,
            observer: None,
            record: None,
            device: Device::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Device {
    pub model: String,
    pub system_version: String,
    pub app_version: String,
    pub lang_code: String,
}

impl Default for Device {
    fn default() -> Self {
        Device {
            model: "vaegram".into(),
            system_version: std::env::consts::OS.into(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            lang_code: "en".into(),
        }
    }
}

pub struct Connection {
    reader: Reader,
    writer: Writer,
}

impl Connection {
    pub async fn connect(dc: &Dc, options: &Options) -> Result<Self> {
        let addr = dc.addr;
        let via = if options.proxy.is_some() { " through the proxy" } else { "" };
        tokio::time::timeout(CONNECT_TIMEOUT, Self::open(dc, options))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, format!("connecting to {addr}{via} timed out")))?
    }

    async fn open(dc: &Dc, options: &Options) -> Result<Self> {
        let addr = dc.addr;
        let stream = match &options.proxy {
            None => TcpStream::connect(addr).await?,
            Some(proxy) => {
                let mut stream = TcpStream::connect(proxy.endpoint()).await?;
                proxy.handshake(&mut stream, addr).await?;
                stream
            }
        };
        let secret = match &options.proxy {
            Some(Proxy::MtProxy { secret, .. }) => Some(secret),
            _ => None,
        };
        let transport =
            if secret.is_some_and(|s| s.padded) { Transport::PaddedIntermediate } else { options.transport };
        stream.set_nodelay(true)?;
        let tls = secret.is_some_and(|s| s.domain.is_some());
        let (read, mut write) = stream.into_split();
        let tag = transport.tag();
        let (encrypt, decrypt, records) = if options.obfuscated || secret.is_some() {
            let (header, encrypt, decrypt) = obfuscated::init(tag, dc.wire_id() as i16, secret.map(|s| &s.key));
            // Behind fake TLS the header rides in the first record together with the first packet.
            let records = tls.then(|| RecordWriter::new(header));
            if !tls {
                write.write_all(&header).await?;
            }
            (Some(encrypt), Some(decrypt), records)
        } else {
            write.write_all(if transport == Transport::Abridged { &tag[..1] } else { &tag[..] }).await?;
            (None, None, None)
        };
        Ok(Self {
            reader: Reader {
                wire: Wire { half: read, records: tls.then(RecordReader::default) },
                transport,
                decrypt,
                buf: RecvBuf::default(),
            },
            writer: Writer { half: write, transport, encrypt, records, buf: Vec::new() },
        })
    }

    pub async fn send(&mut self, payload: &[u8]) -> Result<()> {
        self.writer.send(payload).await
    }

    pub async fn recv(&mut self) -> Result<Vec<u8>> {
        self.reader.recv().await
    }

    pub(crate) fn into_split(self) -> (Reader, Writer) {
        (self.reader, self.writer)
    }
}

pub(crate) struct Reader {
    wire: Wire,
    transport: Transport,
    decrypt: Option<Ctr>,
    buf: RecvBuf,
}

impl Reader {
    /// Cancel-safe: bytes wait in the buffer until a whole packet is there.
    pub(crate) async fn recv(&mut self) -> Result<Vec<u8>> {
        loop {
            if let Some(packet) = self.transport.decode(&mut self.buf)? {
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
    half: OwnedReadHalf,
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

pub(crate) struct Writer {
    half: OwnedWriteHalf,
    transport: Transport,
    encrypt: Option<Ctr>,
    records: Option<RecordWriter>,
    pub(crate) buf: Vec<u8>,
}

impl Writer {
    pub(crate) fn begin(&mut self) {
        self.buf.clear();
        self.buf.resize(Transport::ROOM, 0);
    }

    pub(crate) async fn finish(&mut self) -> Result<()> {
        let start = self.transport.frame(&mut self.buf);
        if let Some(encrypt) = &mut self.encrypt {
            encrypt.apply(&mut self.buf[start..]);
        }
        let wire = match &mut self.records {
            Some(records) => records.wrap(&self.buf[start..]),
            None => &self.buf[start..],
        };
        self.half.write_all(wire).await?;
        Ok(())
    }

    pub(crate) async fn send(&mut self, payload: &[u8]) -> Result<()> {
        self.begin();
        self.buf.extend_from_slice(payload);
        self.finish().await
    }
}
