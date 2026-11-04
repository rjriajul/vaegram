mod fake_tls;
mod obfuscated;
mod proxy;
mod transport;

pub use proxy::{Proxy, Secret};
pub use transport::{Framing, RecvBuf, Transport};

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

use crate::errors::{Error, Result};
use crate::session::Observer;
use crate::session::internals::data_center::Dc;
use crate::storage::recording::Recorder;
use crate::types::Json;
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

#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct Device {
    pub model: String,
    pub system_version: String,
    pub app_version: String,
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
        let secret = match &options.proxy {
            Some(Proxy::MtProxy { secret, .. }) => Some(secret),
            _ => None,
        };
        // The full transport sends no header to hide, so obfuscation has nothing to wrap, and an MTProxy only
        // understands the obfuscated ones.
        if options.transport == Transport::Full && (options.obfuscated || secret.is_some()) {
            return Err(Error::InvalidArgument("the full transport can't be obfuscated or go through an MTProxy"));
        }
        let stream = match &options.proxy {
            None => TcpStream::connect(addr).await?,
            Some(proxy) => {
                let mut stream = TcpStream::connect(proxy.endpoint()).await?;
                proxy.handshake(&mut stream, addr).await?;
                stream
            }
        };
        let transport =
            if secret.is_some_and(|s| s.padded) { Transport::PaddedIntermediate } else { options.transport };
        stream.set_nodelay(true)?;
        let tls = secret.is_some_and(|s| s.domain.is_some());
        let (read, mut write) = stream.into_split();
        let (encrypt, decrypt, records) = match transport.tag() {
            Some(tag) if options.obfuscated || secret.is_some() => {
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
            writer: Writer { half: write, framing, encrypt, records, buf: Vec::new() },
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
    framing: Framing,
    encrypt: Option<Ctr>,
    records: Option<RecordWriter>,
    pub(crate) buf: Vec<u8>,
}

impl Writer {
    pub(crate) fn begin(&mut self) {
        self.buf.clear();
        self.buf.resize(Framing::ROOM, 0);
    }

    pub(crate) async fn finish(&mut self) -> Result<()> {
        let start = self.framing.frame(&mut self.buf);
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

#[cfg(test)]
mod tests {
    use tokio::net::TcpListener;

    use super::*;

    fn full() -> Options {
        Options { transport: Transport::Full, ..Options::default() }
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
}
