use std::io::SeekFrom;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::AtomicI32;

use tokio::io::{AsyncSeekExt, AsyncWriteExt};

use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::session::Session;
use crate::session::transfer::{invoke_part, take, try_join_all, workers};
use crate::types::FileLocation;

// https://core.telegram.org/api/files#downloading-files: a limit that divides 1 MB, at offsets that are
// multiples of it, never crosses a 1 MB boundary.
const CHUNK: i32 = 512 * 1024;
const IN_FLIGHT: u64 = 2 << 20;
const MAX_MIGRATIONS: usize = 3;
// The size comes from the server, so in-memory downloads don't trust it for preallocation.
const MAX_PREALLOC: usize = 64 << 20;

/// Awaiting it collects the file in memory; `.file_name(path)` streams to disk.
#[must_use = "a request does nothing until it is awaited"]
pub struct DownloadMedia<'a> {
    client: &'a Client,
    location: Result<FileLocation>,
}

impl Client {
    /// `media` is a read type such as [`Photo`](crate::types::Photo), a [`FileLocation`] or a `file_id` string.
    pub fn download_media(&self, media: impl TryInto<FileLocation, Error: Into<Error>>) -> DownloadMedia<'_> {
        DownloadMedia::new(self, media.try_into().map_err(Into::into))
    }
}

impl<'a> DownloadMedia<'a> {
    pub(crate) fn new(client: &'a Client, location: Result<FileLocation>) -> Self {
        DownloadMedia { client, location }
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

    async fn download(self, sink: &Sink) -> Result<()> {
        let location = self.location?;
        let size = location.file_size;
        let count = chunk_count(size);
        // Without a size the end is only known from a short chunk, so chunks go one at a time.
        let workers = count.map_or(1, |c| workers(c, CHUNK as u64, IN_FLIGHT));
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
            let lease = self.client.transfer_sessions(dc_id, workers).await?;
            match get_file(&lease.sessions[0], &request).await {
                Err(Error::Rpc(e)) if e.name == "FILE_MIGRATE" && migrations < MAX_MIGRATIONS => {
                    dc_id = e.value.ok_or(Error::Unexpected("FILE_MIGRATE without a DC number"))? as i32;
                    migrations += 1;
                }
                first => break (lease, first?),
            }
        };
        fits(0, &first, size)?;
        sink.write_at(0, &first).await?;
        if first.len() < CHUNK as usize {
            return Ok(());
        }

        let (next, sessions) = (AtomicI32::new(1), &lease.sessions);
        let chunks = (0..workers).map(|i| chunks(&sessions[i % sessions.len()], request.clone(), &next, size, sink));
        try_join_all(chunks).await?;
        Ok(())
    }
}

async fn chunks(
    session: &Session,
    mut request: functions::upload::GetFile,
    next: &AtomicI32,
    size: Option<i64>,
    sink: &Sink,
) -> Result<()> {
    while let Some(n) = take(next, chunk_count(size).unwrap_or(i32::MAX)) {
        (request.offset, request.limit) = chunk(n);
        let bytes = get_file(session, &request).await?;
        fits(request.offset, &bytes, size)?;
        sink.write_at(request.offset as u64, &bytes).await?;
        if bytes.len() < CHUNK as usize {
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
    type IntoFuture = std::pin::Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

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
}

impl Sink {
    async fn write_at(&self, offset: u64, bytes: &[u8]) -> Result<()> {
        match self {
            Sink::Memory(out) => {
                let (mut out, start) = (out.lock().unwrap(), offset as usize);
                if out.len() < start + bytes.len() {
                    out.resize(start + bytes.len(), 0);
                }
                out[start..start + bytes.len()].copy_from_slice(bytes);
            }
            Sink::File(file) => {
                let mut file = file.lock().await;
                file.seek(SeekFrom::Start(offset)).await?;
                file.write_all(bytes).await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::AtomicI32;

    use super::{CHUNK, Sink, chunk, chunk_count, fits};
    use crate::session::transfer::take;

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
            sink.write_at(offset, chunk).await.unwrap();
        }
        let Sink::Memory(out) = sink else { unreachable!() };
        assert_eq!(out.into_inner().unwrap(), b"abcdefgh");
    }
}
