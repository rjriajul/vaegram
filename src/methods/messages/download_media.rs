use std::future::poll_fn;
use std::io::SeekFrom;
use std::path::Path;
use std::pin::Pin;
use std::sync::Mutex;
use std::sync::atomic::AtomicI32;
use std::task::Poll;

use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{mpsc, watch};

use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::session::Session;
use crate::session::transfer::{Progress, Report, invoke_part, take, try_join_all, workers};
use crate::types::FileLocation;

// https://core.telegram.org/api/files#downloading-files: a limit that divides 1 MB, at offsets that are
// multiples of it, never crosses a 1 MB boundary.
const CHUNK: i32 = 512 * 1024;
const IN_FLIGHT: u64 = 2 << 20;
// The server throttles Premium downloads less, so they get 8 times the budget.
const IN_FLIGHT_PREMIUM: u64 = 16 << 20;
const MAX_CONNECTIONS: usize = 8;
const MAX_MIGRATIONS: usize = 3;
// The size comes from the server, so in-memory downloads don't trust it for preallocation.
const MAX_PREALLOC: usize = 64 << 20;
const END: u64 = u64::MAX;

/// Awaiting it collects the file in memory; `.file_name(path)` writes it to disk, `.stream()` yields chunks.
#[must_use = "a request does nothing until it is awaited"]
pub struct DownloadMedia<'a> {
    client: &'a Client,
    location: Result<FileLocation>,
    progress: Option<Report<'a>>,
}

#[must_use = "a stream does nothing until `next()` is awaited"]
pub struct MediaStream<'a> {
    download: Option<Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>>,
    chunks: mpsc::Receiver<Vec<u8>>,
}

impl Client {
    /// `media` is a read type such as [`Photo`](crate::types::Photo), a [`FileLocation`] or a `file_id` string.
    pub fn download_media(&self, media: impl TryInto<FileLocation, Error: Into<Error>>) -> DownloadMedia<'_> {
        DownloadMedia::new(self, media.try_into().map_err(Into::into))
    }
}

impl<'a> DownloadMedia<'a> {
    pub(crate) fn new(client: &'a Client, location: Result<FileLocation>) -> Self {
        DownloadMedia { client, location, progress: None }
    }

    /// Calls `progress(done, total)` with byte counts after each chunk. `total` is 0 when Telegram gave no size.
    pub fn progress(mut self, progress: impl FnMut(u64, u64) + Send + 'a) -> Self {
        self.progress = Some(Box::new(progress));
        self
    }

    /// Streams the file to `path` instead, creating missing parent directories. Don't pass a name taken from the
    /// message as is: the sender chose it, and it can contain `..` or an absolute path.
    pub async fn file_name(self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            tokio::fs::create_dir_all(dir).await?;
        }
        let sink = Sink::File(tokio::sync::Mutex::new(tokio::fs::File::create(path).await?));
        let mut written = self.download(&sink).await;
        if let (Ok(()), Sink::File(file)) = (&written, &sink) {
            written = file.lock().await.flush().await.map_err(Error::from);
        }
        drop(sink);
        if written.is_err() {
            let _ = tokio::fs::remove_file(path).await;
        }
        written
    }

    /// Yields the file in order while later chunks download; it only downloads while `next()` is awaited.
    pub fn stream(self) -> MediaStream<'a> {
        let (tx, chunks) = mpsc::channel(1);
        let download = async move {
            let sink = Sink::Stream { sent: watch::channel(0).0, chunks: tx };
            self.download(&sink).await
        };
        MediaStream { download: Some(Box::pin(download)), chunks }
    }

    async fn download(self, sink: &Sink) -> Result<()> {
        let location = self.location?;
        let size = location.file_size;
        let count = chunk_count(size);
        let progress = Progress::new(self.progress, size.and_then(|s| u64::try_from(s).ok()).unwrap_or(0));
        let budget = if self.client.is_premium().await.unwrap_or(false) { IN_FLIGHT_PREMIUM } else { IN_FLIGHT };
        // Without a size the end is only known from a short chunk, so chunks go one at a time.
        let workers = count.map_or(1, |c| workers(c, CHUNK as u64, budget));
        let request = functions::upload::GetFile {
            precise: false,
            cdn_supported: false,
            location: location.location,
            offset: 0,
            limit: CHUNK,
        };

        let mut dc_id = location.dc_id.unwrap_or_else(|| self.client.session().dc_id());
        let mut migrations = 0;
        let (lease, first) = loop {
            let lease = self.client.transfer_sessions(dc_id, workers.min(MAX_CONNECTIONS)).await?;
            match get_file(&lease.sessions[0], &request).await {
                Err(Error::Rpc(e)) if e.name == "FILE_MIGRATE" && migrations < MAX_MIGRATIONS => {
                    dc_id = e.value.ok_or(Error::Unexpected("FILE_MIGRATE without a DC number"))? as i32;
                    migrations += 1;
                }
                first => break (lease, first?),
            }
        };
        fits(0, &first, size)?;
        let len = first.len();
        sink.write_at(0, first).await?;
        progress.add(len);
        if len < CHUNK as usize {
            return Ok(());
        }

        let (next, sessions) = (AtomicI32::new(1), &lease.sessions);
        let chunks =
            (0..workers).map(|i| chunks(&sessions[i % sessions.len()], request.clone(), &next, size, sink, &progress));
        try_join_all(chunks).await?;
        Ok(())
    }
}

impl MediaStream<'_> {
    pub async fn next(&mut self) -> Option<Result<Vec<u8>>> {
        poll_fn(|cx| {
            if let Some(download) = &mut self.download {
                if let Poll::Ready(done) = download.as_mut().poll(cx) {
                    self.download = None;
                    if let Err(e) = done {
                        while self.chunks.try_recv().is_ok() {}
                        return Poll::Ready(Some(Err(e)));
                    }
                }
            }
            self.chunks.poll_recv(cx).map(|chunk| chunk.map(Ok))
        })
        .await
    }
}

async fn chunks(
    session: &Session,
    mut request: functions::upload::GetFile,
    next: &AtomicI32,
    size: Option<i64>,
    sink: &Sink,
    progress: &Progress<'_>,
) -> Result<()> {
    while let Some(n) = take(next, chunk_count(size).unwrap_or(i32::MAX)) {
        (request.offset, request.limit) = chunk(n);
        let bytes = get_file(session, &request).await?;
        fits(request.offset, &bytes, size)?;
        let len = bytes.len();
        sink.write_at(request.offset as u64, bytes).await?;
        progress.add(len);
        if len < CHUNK as usize {
            break;
        }
    }
    Ok(())
}

fn chunk_count(size: Option<i64>) -> Option<i32> {
    size.and_then(|s| u64::try_from(s).ok()).and_then(|s| i32::try_from(s.div_ceil(CHUNK as u64)).ok())
}

fn fits(offset: i64, bytes: &[u8], size: Option<i64>) -> Result<()> {
    let end = offset.saturating_add(bytes.len() as i64);
    if bytes.len() > CHUNK as usize || size.is_some_and(|s| end > s) {
        return Err(Error::Unexpected("upload.getFile returned more than the file size"));
    }
    Ok(())
}

fn chunk(n: i32) -> (i64, i32) {
    (i64::from(n) * i64::from(CHUNK), CHUNK)
}

impl<'a> IntoFuture for DownloadMedia<'a> {
    type Output = Result<Vec<u8>>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            let capacity = self.location.as_ref().ok().and_then(|l| usize::try_from(l.file_size?).ok());
            let capacity = capacity.unwrap_or(0).min(MAX_PREALLOC);
            let sink = Sink::Memory(Mutex::new(Vec::with_capacity(capacity)));
            self.download(&sink).await?;
            let Sink::Memory(out) = sink else { unreachable!() };
            Ok(out.into_inner().unwrap())
        })
    }
}

async fn get_file(session: &Session, request: &functions::upload::GetFile) -> Result<Vec<u8>> {
    match invoke_part(session, request).await? {
        base::upload::File::File(f) => Ok(f.bytes),
        // Asking without cdn_supported keeps files off CDN DCs.
        base::upload::File::FileCdnRedirect(_) => Err(Error::Unexpected("upload.getFile redirected to a CDN")),
    }
}

enum Sink {
    Memory(Mutex<Vec<u8>>),
    File(tokio::sync::Mutex<tokio::fs::File>),
    // Each chunk waits for the ones before it, so a slow chunk holds back at most one per worker.
    Stream { sent: watch::Sender<u64>, chunks: mpsc::Sender<Vec<u8>> },
}

impl Sink {
    async fn write_at(&self, offset: u64, bytes: Vec<u8>) -> Result<()> {
        match self {
            Sink::Memory(out) => {
                let (mut out, start) = (out.lock().unwrap(), offset as usize);
                if out.len() < start + bytes.len() {
                    out.resize(start + bytes.len(), 0);
                }
                out[start..start + bytes.len()].copy_from_slice(&bytes);
            }
            Sink::File(file) => {
                let mut file = file.lock().await;
                file.seek(SeekFrom::Start(offset)).await?;
                file.write_all(&bytes).await?;
            }
            Sink::Stream { sent, chunks } => {
                let mut turn = sent.subscribe();
                let at = *turn.wait_for(|&s| s == offset || s == END).await.unwrap();
                if at == END {
                    return Ok(());
                }
                let len = bytes.len() as u64;
                if len > 0 {
                    let _ = chunks.send(bytes).await;
                }
                sent.send_replace(if len < CHUNK as u64 { END } else { offset + len });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::AtomicI32;

    use tokio::sync::{mpsc, watch};

    use super::{CHUNK, MediaStream, Sink, chunk, chunk_count, fits};
    use crate::errors::Error;
    use crate::session::transfer::{take, try_join_all};

    #[test]
    fn asks_for_aligned_chunks_that_cover_the_file() {
        const MB: i64 = 1 << 20;
        assert_eq!(chunk_count(None), None);
        assert_eq!(chunk_count(Some(0)), Some(0));
        assert_eq!(chunk_count(Some(1)), Some(1));
        assert_eq!(chunk_count(Some(MB)), Some(2));

        let size = 5 * MB / 2 + 1;
        let (count, next) = (chunk_count(Some(size)).unwrap(), AtomicI32::new(0));
        let mut requests: Vec<_> = std::iter::from_fn(|| take(&next, count)).map(chunk).collect();
        requests.sort();
        assert_eq!(requests.iter().map(|r| r.0).collect::<Vec<_>>(), (0..6).map(|n| n * MB / 2).collect::<Vec<_>>());
        for (offset, limit) in requests.iter().copied() {
            assert_eq!((offset % 4096, limit % 4096, MB % i64::from(limit)), (0, 0, 0));
            assert_eq!(offset / MB, (offset + i64::from(limit) - 1) / MB);
        }
        let last = requests.last().unwrap();
        assert!(last.0 < size && size <= last.0 + i64::from(CHUNK));
    }

    #[test]
    fn rejects_sizes_and_chunks_a_server_could_inflate() {
        assert_eq!(chunk_count(Some(i64::MAX)), None);
        assert_eq!(chunk_count(Some(-1)), None);
        let chunk = vec![0; CHUNK as usize];
        assert!(fits(0, &chunk, Some(i64::from(CHUNK))).is_ok());
        assert!(fits(0, &chunk, None).is_ok());
        assert!(fits(1, &chunk, Some(i64::from(CHUNK))).is_err());
        assert!(fits(0, &vec![0; CHUNK as usize + 1], None).is_err());
    }

    #[tokio::test]
    async fn writes_chunks_at_their_offsets() {
        let sink = Sink::Memory(Mutex::new(Vec::new()));
        for (offset, chunk) in [(6, &b"gh"[..]), (0, b"abc"), (3, b"def")] {
            sink.write_at(offset, chunk.to_vec()).await.unwrap();
        }
        let Sink::Memory(out) = sink else { unreachable!() };
        assert_eq!(out.into_inner().unwrap(), b"abcdefgh");
    }

    #[tokio::test]
    async fn streams_chunks_in_file_order_whatever_order_they_arrive() {
        let c = CHUNK as usize;
        let (tx, chunks) = mpsc::channel(1);
        let download = async move {
            let sink = Sink::Stream { sent: watch::channel(0).0, chunks: tx };
            let arrive = |n: usize, len: usize, delay: usize| {
                let sink = &sink;
                async move {
                    for _ in 0..delay {
                        tokio::task::yield_now().await;
                    }
                    sink.write_at((n * c) as u64, vec![n as u8; len]).await
                }
            };
            // Chunk 3 is short, so it ends the file and chunk 4 is dropped.
            let parts = [arrive(2, c, 0), arrive(4, c, 1), arrive(3, 7, 2), arrive(0, c, 3), arrive(1, c, 6)];
            try_join_all(parts.into_iter()).await.map(drop)
        };
        let mut stream = MediaStream { download: Some(Box::pin(download)), chunks };
        let mut got = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.unwrap();
            got.push((chunk[0], chunk.len()));
        }
        assert_eq!(got, [(0, c), (1, c), (2, c), (3, 7)]);

        let failing = Box::pin(async { Err::<(), _>(Error::Timeout) });
        let mut failed = MediaStream { download: Some(failing), chunks: mpsc::channel(1).1 };
        assert!(matches!(failed.next().await, Some(Err(Error::Timeout))));
        assert!(failed.next().await.is_none());
    }
}
