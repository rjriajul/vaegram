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
        let (stream, secret) = match &options.proxy {
            None => (TcpStream::connect(addr).await?, None),
            Some(Proxy::Socks5 { host, port, auth }) => {
                let mut stream = TcpStream::connect((host.as_str(), *port)).await?;
                proxy::socks5(&mut stream, addr, auth.as_ref()).await?;
                (stream, None)
            }
            Some(Proxy::MtProxy { host, port, secret }) => {
                (TcpStream::connect((host.as_str(), *port)).await?, Some(secret))
            }
        };
        let transport =
            if secret.is_some_and(|s| s.padded) { Transport::PaddedIntermediate } else { options.transport };
        stream.set_nodelay(true)?;
        let (read, mut write) = stream.into_split();
        let tag = transport.tag();
        let (encrypt, decrypt) = if options.obfuscated || secret.is_some() {
            let (header, encrypt, decrypt) = obfuscated::init(tag, dc.wire_id() as i16, secret.map(|s| &s.key));
            write.write_all(&header).await?;
            (Some(encrypt), Some(decrypt))
        } else {
            write.write_all(if transport == Transport::Abridged { &tag[..1] } else { &tag[..] }).await?;
            (None, None)
        };
        Ok(Self {
            reader: Reader { half: read, transport, decrypt },
            writer: Writer { half: write, transport, encrypt, buf: Vec::new() },
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
        ready!(Pin::new(&mut this.half).poll_read(cx, buf))?;
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
    buf: Vec<u8>,
}

impl Writer {
    pub(crate) async fn send(&mut self, payload: &[u8]) -> Result<()> {
        self.buf.clear();
        self.transport.encode(payload, &mut self.buf);
        if let Some(encrypt) = &mut self.encrypt {
            encrypt.apply(&mut self.buf);
        }
        self.half.write_all(&self.buf).await?;
        Ok(())
    }
}
