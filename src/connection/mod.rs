pub mod transport;

use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

use crate::errors::Result;
use transport::intermediate;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct Connection {
    stream: TcpStream,
}

impl Connection {
    pub async fn connect(addr: SocketAddr) -> Result<Self> {
        let mut stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await.map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::TimedOut, format!("connecting to {addr} timed out"))
        })??;
        stream.set_nodelay(true)?;
        stream.write_all(&intermediate::TAG).await?;
        Ok(Self { stream })
    }

    pub async fn send(&mut self, payload: &[u8]) -> Result<()> {
        intermediate::send(&mut self.stream, payload).await
    }

    pub async fn recv(&mut self) -> Result<Vec<u8>> {
        intermediate::recv(&mut self.stream).await
    }

    pub(crate) fn into_split(self) -> (OwnedReadHalf, OwnedWriteHalf) {
        self.stream.into_split()
    }
}
