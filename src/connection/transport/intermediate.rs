use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::errors::{Error, Result};

pub const TAG: [u8; 4] = [0xee; 4];

const MAX_PACKET: usize = 16 << 20;

pub async fn send<W: AsyncWrite + Unpin>(w: &mut W, payload: &[u8]) -> Result<()> {
    let mut packet = Vec::with_capacity(4 + payload.len());
    packet.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    packet.extend_from_slice(payload);
    w.write_all(&packet).await?;
    Ok(())
}

pub async fn recv<R: AsyncRead + Unpin>(r: &mut R) -> Result<Vec<u8>> {
    let len = r.read_u32_le().await? as usize;
    if len > MAX_PACKET {
        return Err(
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{len}-byte packet is too big")).into()
        );
    }
    let mut payload = vec![0; len];
    r.read_exact(&mut payload).await?;
    if let Ok(code) = <[u8; 4]>::try_from(payload.as_slice()).map(i32::from_le_bytes) {
        if code < 0 {
            return Err(Error::Transport(-code));
        }
    }
    Ok(payload)
}
