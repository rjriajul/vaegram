//! Just enough HTTP/1.1 to read a response head, as a WebSocket upgrade answers with.

use std::io;

use tokio::io::{AsyncRead, AsyncReadExt};

// Far more than any upgrade answer; a server that sends more isn't one to talk to.
const MAX_HEAD: usize = 64 << 10;

pub(crate) struct Head {
    pub(crate) status: u16,
    headers: Vec<(String, String)>,
}

impl Head {
    /// The first header called `name`, which is matched without regard to case.
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// Reads up to the end of the response head; returns it and the bytes that already arrived after it.
pub(crate) async fn read_head<S: AsyncRead + Unpin>(stream: &mut S) -> io::Result<(Head, Vec<u8>)> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0; 1024];
    loop {
        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let rest = buf.split_off(end + 4);
            return Ok((parse(&buf[..end])?, rest));
        }
        if buf.len() > MAX_HEAD {
            return Err(invalid("the HTTP response head is too long"));
        }
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the server closed before its response head ended",
            ));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

fn parse(head: &[u8]) -> io::Result<Head> {
    let head = std::str::from_utf8(head).map_err(|_| invalid("the HTTP response head isn't text"))?;
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or_default();
    let status = match status_line.split(' ').collect::<Vec<_>>()[..] {
        [version, code, ..] if version.starts_with("HTTP/1.") => code.parse().ok(),
        _ => None,
    };
    let status = status.ok_or_else(|| invalid(format!("not an HTTP/1 status line: {status_line:?}")))?;
    let headers = lines
        .map(|line| {
            let (name, value) = line.split_once(':').ok_or_else(|| invalid(format!("not a header: {line:?}")))?;
            Ok((name.trim().to_owned(), value.trim().to_owned()))
        })
        .collect::<io::Result<_>>()?;
    Ok(Head { status, headers })
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncWriteExt;

    use super::read_head;

    #[tokio::test]
    async fn reads_the_head_and_keeps_what_follows() {
        let (mut client, mut server) = tokio::io::duplex(64);
        let answer = b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nX-Two:  a:b \r\n\r\n\x82\x01z";
        tokio::spawn(async move { server.write_all(answer).await });
        let (head, rest) = read_head(&mut client).await.unwrap();
        assert_eq!((head.status, head.header("UPGRADE"), head.header("x-two")), (101, Some("websocket"), Some("a:b")));
        assert_eq!((head.header("missing"), rest.as_slice()), (None, &b"\x82\x01z"[..]));
    }

    #[tokio::test]
    async fn refuses_what_isnt_an_http_head() {
        for answer in [&b"SSH-2.0-OpenSSH\r\n\r\n"[..], b"HTTP/1.1 abc\r\n\r\n", b"HTTP/1.1 200 OK\r\nno colon\r\n\r\n"]
        {
            let (mut client, mut server) = tokio::io::duplex(64);
            tokio::spawn(async move { server.write_all(answer).await });
            let kind = read_head(&mut client).await.err().map(|e| e.kind());
            assert_eq!(kind, Some(std::io::ErrorKind::InvalidData), "{answer:?}");
        }
        let (mut client, server) = tokio::io::duplex(64);
        drop(server);
        assert_eq!(read_head(&mut client).await.err().map(|e| e.kind()), Some(std::io::ErrorKind::UnexpectedEof));
    }
}
