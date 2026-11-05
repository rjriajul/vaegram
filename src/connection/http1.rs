//! Just enough HTTP/1.1 to read a response head, as a WebSocket upgrade answers with, and whole responses off a
//! keep-alive connection.

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

/// One whole response off a keep-alive connection. `pending` holds the bytes read past the previous response and
/// keeps those read past this one. The body is delimited by `Content-Length` or chunked framing and holds at most
/// `max_body` bytes; a body that runs to the close can't be told from a dropped connection, so it's refused.
#[cfg(feature = "web-proxy")]
pub(crate) async fn read_response<S: AsyncRead + Unpin>(
    stream: &mut S,
    pending: &mut Vec<u8>,
    max_body: usize,
) -> io::Result<(Head, Vec<u8>)> {
    let end = fill_until(stream, pending, MAX_HEAD, b"\r\n\r\n").await?;
    let head = parse(&pending[..end])?;
    pending.drain(..end + 4);
    let chunked = head.header("transfer-encoding").is_some_and(|v| v.to_ascii_lowercase().contains("chunked"));
    if chunked {
        let mut body = Vec::new();
        loop {
            let end = fill_until(stream, pending, MAX_HEAD, b"\r\n").await?;
            let line = std::str::from_utf8(&pending[..end]).map_err(|_| invalid("a chunk size isn't text"))?;
            let size = line.split(';').next().unwrap_or_default().trim();
            let size = usize::from_str_radix(size, 16).map_err(|_| invalid("a chunk size isn't hex"))?;
            pending.drain(..end + 2);
            if size == 0 {
                // Trailer fields, if any, then the blank line that ends the body.
                loop {
                    let end = fill_until(stream, pending, MAX_HEAD, b"\r\n").await?;
                    pending.drain(..end + 2);
                    if end == 0 {
                        return Ok((head, body));
                    }
                }
            }
            if body.len().saturating_add(size) > max_body {
                return Err(invalid("the HTTP response body is too long"));
            }
            fill_to(stream, pending, size + 2).await?;
            if pending[size..size + 2] != *b"\r\n" {
                return Err(invalid("a chunk doesn't end where its size says"));
            }
            body.extend_from_slice(&pending[..size]);
            pending.drain(..size + 2);
        }
    }
    let body = match head.header("content-length") {
        Some(length) => {
            let length: usize = length.parse().map_err(|_| invalid("a bad Content-Length"))?;
            if length > max_body {
                return Err(invalid("the HTTP response body is too long"));
            }
            fill_to(stream, pending, length).await?;
            pending.drain(..length).collect()
        }
        None if matches!(head.status, 204 | 304) => Vec::new(),
        None => return Err(invalid("an HTTP response has neither Content-Length nor chunked framing")),
    };
    Ok((head, body))
}

/// Reads into `pending` until `needle` is in it, within `limit` bytes; returns where it starts.
#[cfg(feature = "web-proxy")]
async fn fill_until<S: AsyncRead + Unpin>(
    stream: &mut S,
    pending: &mut Vec<u8>,
    limit: usize,
    needle: &[u8],
) -> io::Result<usize> {
    loop {
        if let Some(at) = pending.windows(needle.len()).position(|w| w == needle) {
            return Ok(at);
        }
        if pending.len() > limit {
            return Err(invalid("an HTTP line is too long"));
        }
        read_more(stream, pending).await?;
    }
}

#[cfg(feature = "web-proxy")]
async fn fill_to<S: AsyncRead + Unpin>(stream: &mut S, pending: &mut Vec<u8>, len: usize) -> io::Result<()> {
    while pending.len() < len {
        read_more(stream, pending).await?;
    }
    Ok(())
}

#[cfg(feature = "web-proxy")]
async fn read_more<S: AsyncRead + Unpin>(stream: &mut S, pending: &mut Vec<u8>) -> io::Result<()> {
    let mut chunk = [0; 4096];
    let n = stream.read(&mut chunk).await?;
    if n == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "the server closed inside an HTTP response"));
    }
    pending.extend_from_slice(&chunk[..n]);
    Ok(())
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

    #[cfg(feature = "web-proxy")]
    #[tokio::test]
    async fn reads_keep_alive_responses_one_after_another() {
        use super::read_response;
        let answers = concat!(
            "HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nabc",
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2;x=y\r\nde\r\n1\r\nf\r\n0\r\nTrailer: z\r\n\r\n",
            "HTTP/1.1 204 No Content\r\n\r\n",
            "HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\ng",
        );
        let (mut client, mut server) = tokio::io::duplex(16);
        tokio::spawn(async move { server.write_all(answers.as_bytes()).await });
        let mut pending = Vec::new();
        let mut bodies = Vec::new();
        for _ in 0..4 {
            let (head, body) = read_response(&mut client, &mut pending, 64).await.unwrap();
            bodies.push((head.status, String::from_utf8(body).unwrap()));
        }
        let want = [(200, "abc"), (200, "def"), (204, ""), (200, "g")];
        assert_eq!(bodies, want.map(|(s, b)| (s, b.to_owned())));
        assert!(pending.is_empty());
    }

    #[cfg(feature = "web-proxy")]
    #[tokio::test]
    async fn refuses_bodies_it_cant_delimit_or_that_run_long() {
        use super::read_response;
        for answer in [
            &b"HTTP/1.1 200 OK\r\n\r\nrest"[..],
            b"HTTP/1.1 200 OK\r\nContent-Length: 65\r\n\r\n",
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n41\r\n",
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\nab\r\n",
        ] {
            let (mut client, mut server) = tokio::io::duplex(256);
            tokio::spawn(async move { server.write_all(answer).await });
            let kind = read_response(&mut client, &mut Vec::new(), 64).await.err().map(|e| e.kind());
            assert_eq!(kind, Some(std::io::ErrorKind::InvalidData), "{answer:?}");
        }
    }
}
