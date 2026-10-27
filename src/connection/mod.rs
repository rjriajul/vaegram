mod fake_tls;
mod obfuscated;
mod proxy;
mod transport;

pub use proxy::{Proxy, Secret};
pub use transport::Transport;

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

use crate::errors::Result;
use crate::session::Observer;
use crate::session::internals::data_center::Dc;
use crate::storage::recording::Recorder;
use fake_tls::{RecordReader, RecordWriter};
use obfuscated::Ctr;

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
    pub observer: Option<Observer>,
    pub record: Option<Recorder>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            transport: Transport::default(),
            obfuscated: false,
            proxy: None,
            request_timeout: Duration::from_secs(30),
            flood_sleep_threshold: Duration::from_secs(60),
            observer: None,
            record: None,
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
            reader: Reader { half: read, transport, decrypt, records: tls.then(RecordReader::default) },
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
    half: OwnedReadHalf,
    transport: Transport,
    decrypt: Option<Ctr>,
    records: Option<RecordReader>,
}

impl Reader {
    pub(crate) async fn recv(&mut self) -> Result<Vec<u8>> {
        let transport = self.transport;
        transport.read(self).await
    }
}

impl AsyncRead for Reader {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let start = buf.filled().len();
        let this = &mut *self;
        match &mut this.records {
            Some(records) => ready!(records.poll_read(&mut this.half, cx, buf))?,
            None => ready!(Pin::new(&mut this.half).poll_read(cx, buf))?,
        }
        if let Some(decrypt) = &mut this.decrypt {
            decrypt.apply(&mut buf.filled_mut()[start..]);
        }
        Poll::Ready(Ok(()))
    }
}

pub(crate) struct Writer {
    half: OwnedWriteHalf,
    transport: Transport,
    encrypt: Option<Ctr>,
    records: Option<RecordWriter>,
    buf: Vec<u8>,
}

impl Writer {
    pub(crate) async fn send(&mut self, payload: &[u8]) -> Result<()> {
        self.buf.clear();
        self.transport.encode(payload, &mut self.buf);
        if let Some(encrypt) = &mut self.encrypt {
            encrypt.apply(&mut self.buf);
        }
        let wire = match &mut self.records {
            Some(records) => records.wrap(&self.buf),
            None => &self.buf,
        };
        self.half.write_all(wire).await?;
        Ok(())
    }
}
