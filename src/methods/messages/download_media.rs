use std::future::poll_fn;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::Duration;

use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{mpsc, watch};

use crate::Client;
use crate::crypto::random;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::session::Session;
use crate::session::cdn::Cdn;
use crate::session::pacer::{Direction, MAX_CONNECTIONS, Pace, Pacer, Shape};
use crate::session::transfer::{Progress, Report, part_timeout, take, try_join_all, workers};
use crate::types::FileLocation;

// https://core.telegram.org/api/files#downloading-files: a limit that divides 1 MB, at offsets that are
// multiples of it, never crosses a 1 MB boundary.
const CHUNK: i32 = 512 * 1024;
const MAX_MIGRATIONS: usize = 3;
// Telegram's largest file, 8000 parts of 512 KiB. Reserving the declared size up to this once avoids the copies and
// the 2x capacity of growing by doubling; a size the server overstates costs at most this, and fails as an Err.
const MAX_PREALLOC: usize = 8000 * 512 * 1024;
const TOO_BIG: Error = Error::Unexpected("the file is too big to hold in memory");
const END: u64 = u64::MAX;

fn out_of_memory() -> Error {
    Error::Io(std::io::ErrorKind::OutOfMemory.into())
}

/// A file written beside its destination; dropped without `rename_to`, it deletes itself, so an error and a
/// cancelled future both leave nothing behind.
struct TempFile(Option<PathBuf>);

impl TempFile {
    fn create(destination: &Path) -> std::io::Result<(TempFile, std::fs::File)> {
        let mut name = destination.as_os_str().to_owned();
        name.push(format!(".{:016x}.part", u64::from_le_bytes(random())));
        let path = PathBuf::from(name);
        let file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path)?;
        Ok((TempFile(Some(path)), file))
    }

    async fn rename_to(mut self, destination: &Path) -> Result<()> {
        if let Some(path) = &self.0 {
            tokio::fs::rename(path, destination).await?;
            self.0 = None;
        }
        Ok(())
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Where a file came from, re-read for a fresh file_reference once it expires.
#[derive(Debug, Clone, Copy)]
pub(crate) enum DownloadSource {
    /// A chat and message id.
    Message(i64, i32),
    /// A chat and story id.
    Story(i64, i32),
}

/// Awaiting it collects the file in memory; `.save(path)` writes it to disk, `.stream()` yields chunks.
#[must_use = "a request does nothing until it is awaited"]
pub struct DownloadMedia<'a> {
    client: &'a Client,
    location: Result<FileLocation>,
    source: Option<DownloadSource>,
    progress: Option<Report<'a>>,
    offset: u64,
    limit: Option<u64>,
}

/// Chunks of a download, in file order. Get one from [`DownloadMedia::stream`].
#[must_use = "a stream does nothing until `next()` is awaited"]
pub struct MediaStream<'a> {
    download: Option<Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>>,
    chunks: mpsc::Receiver<Vec<u8>>,
}

impl Client {
    /// Downloads a file from Telegram.
    ///
    /// `media` is a read type such as [`Photo`](crate::types::Photo), a [`FileLocation`] or a `file_id` string.
    /// Awaiting the request returns the whole file as bytes. A file id that doesn't parse fails the request before
    /// anything is sent. An expired file reference fails the download.
    /// [`Message::download`](crate::types::Message::download) and `Story::download` re-read their source once and
    /// retry.
    pub fn download_media(&self, media: impl TryInto<FileLocation, Error: Into<Error>>) -> DownloadMedia<'_> {
        DownloadMedia::new(self, media.try_into().map_err(Into::into), None)
    }
}

impl<'a> DownloadMedia<'a> {
    pub(crate) fn new(client: &'a Client, location: Result<FileLocation>, source: Option<DownloadSource>) -> Self {
        DownloadMedia { client, location, source, progress: None, offset: 0, limit: None }
    }

    /// Calls `progress(done, total)` with byte counts after each chunk. `total` is 0 when Telegram gave no size.
    pub fn progress(mut self, progress: impl FnMut(u64, u64) + Send + 'a) -> Self {
        self.progress = Some(Box::new(progress));
        self
    }

    /// Starts at this byte of the file, for a byte range together with [`limit`](Self::limit). An offset at or past
    /// the end gives no bytes.
    pub fn offset(mut self, offset: u64) -> Self {
        self.offset = offset;
        self
    }

    /// Gives at most this many bytes. A range that runs past the end of the file is cut there; 0 gives no bytes and
    /// sends no request.
    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Streams the file to `path` instead, creating missing parent directories. An existing file at `path` is
    /// replaced only when the download succeeds, by a new file: a symlink or the permissions of the old one are not
    /// kept. A failed or cancelled download leaves it as it was, and a `path` that is a directory is refused. Don't
    /// pass a name taken from the message as is: the sender chose it, and it can contain `..` or an absolute path.
    pub async fn save(self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        // A bad file id fails before anything touches the disk.
        if self.location.is_err() {
            return self.location.map(drop);
        }
        if path.is_dir() {
            return Err(Error::InvalidArgument("the destination is a directory"));
        }
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        // The file is written next to its destination, so the final rename stays on one filesystem.
        let (temp, file) = TempFile::create(path)?;
        let sink = Sink::File(tokio::sync::Mutex::new(tokio::fs::File::from_std(file)));
        self.download(&sink).await?;
        if let Sink::File(file) = &sink {
            file.lock().await.flush().await?;
        }
        // A file that is still open can't be renamed over on Windows.
        drop(sink);
        temp.rename_to(path).await
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
        let client = self.client;
        let location = self.location?;
        let size = location.file_size;
        let Some(window) = Window::new(self.offset, self.limit, size.and_then(|s| u64::try_from(s).ok())) else {
            return Ok(());
        };
        // Held until the download ends; a stream keeps it until it's dropped or finishes.
        let _slot = client.download_slot().await;
        let progress = Progress::new(self.progress, window.len());
        let premium = client.is_premium().await.unwrap_or(false);
        // A bounded range of a big file is as small as the bytes it asks for; unbounded with no size, it counts as
        // large.
        let plan = client.transfer_plan(Direction::Download, premium, Some(window.len()).filter(|&len| len > 0));
        // Without a size the end is only known from a short chunk, so chunks go one at a time.
        let parts = window.bound.saturating_sub(window.first);
        let workers_for =
            |shape: Shape| chunk_count(size).map_or(1, |_| workers(parts, CHUNK as u64, shape.in_flight(CHUNK as u64)));
        let workers = workers_for(plan.shape);
        let mut request = functions::upload::GetFile {
            precise: false,
            // A CDN connection can't carry the takeout wrapper, and CDNs have no WebSocket endpoint.
            cdn_supported: client.1.is_none() && !client.0.options.uses_websocket(),
            location: location.location,
            offset: window.first_offset(),
            limit: CHUNK,
        };
        if let Sink::Stream { sent, .. } = sink {
            sent.send_replace(window.first_offset() as u64);
        }
        let mut dc_id = location.dc_id.unwrap_or_else(|| client.session().dc_id());
        let (mut migrations, mut refreshed) = (0, false);
        let (lease, pacer, file) = loop {
            let lease = client.transfer_sessions(client.media_dc(dc_id)?, workers.min(plan.shape.connections)).await?;
            let pacer = client.pacer(dc_id, Direction::Download, plan);
            let pace = Pace { pacer: pacer.as_deref(), bytes: chunk_cost(request.offset, size), gated: plan.gated };
            // The first chunk goes out alone.
            match client.invoke_file_part(&lease.sessions[0], &request, part_timeout(CHUNK as u64), pace).await {
                Ok(file) => break (lease, pacer, file),
                Err(Error::Rpc(e)) if e.is("FILE_MIGRATE") && migrations < MAX_MIGRATIONS => {
                    dc_id = e.migrate_dc().ok_or(Error::Unexpected("FILE_MIGRATE without a DC number"))?;
                    migrations += 1;
                }
                Err(Error::Rpc(e)) if e.name.starts_with("FILE_REFERENCE_") && !refreshed => {
                    refreshed = true;
                    if !refresh(client, self.source, &mut request.location).await? {
                        return Err(Error::Rpc(e));
                    }
                }
                Err(e) => return Err(e),
            }
        };
        let master = Master {
            // A connection carries its share of the workers' chunks at once, more of them when it opened fewer.
            timeout: part_timeout(workers.div_ceil(lease.sessions.len()) as u64 * CHUNK as u64),
            client,
            sessions: &lease.sessions,
            source: self.source,
            location: Mutex::new((request.location, 0)),
            renewing: tokio::sync::Mutex::new(()),
            pacer,
            gated: plan.gated,
            size,
        };
        let master = &master;
        match file {
            base::upload::File::File(f) => {
                let part = move |i, offset| master.part(i, offset);
                write_all(f.bytes, &window, size, workers, sink, &progress, part).await
            }
            base::upload::File::FileCdnRedirect(r) => {
                let workers = workers_for(client.0.transfer_settings.cdn_shape(premium));
                from_cdn(master, *r, &window, size, workers, sink, &progress).await
            }
        }
    }
}

/// The DC that stores the file. Chunks share its file_reference, renewed once when several expire together.
struct Master<'a> {
    client: &'a Client,
    sessions: &'a [Session],
    source: Option<DownloadSource>,
    location: Mutex<(base::InputFileLocation, usize)>,
    renewing: tokio::sync::Mutex<()>,
    timeout: Duration,
    // Shared with every download from this DC; `size` tells it a short last chunk.
    pacer: Option<Arc<Pacer>>,
    // Whether each getFile holds its connection's gate; see `Pace`.
    gated: bool,
    size: Option<i64>,
}

impl Master<'_> {
    async fn part(&self, worker: usize, offset: i64) -> Result<Vec<u8>> {
        let session = &self.sessions[worker % self.sessions.len()];
        let mut renewed = false;
        loop {
            let (location, seen) = self.location.lock().unwrap().clone();
            let request =
                functions::upload::GetFile { precise: false, cdn_supported: false, location, offset, limit: CHUNK };
            let pace = Pace { pacer: self.pacer.as_deref(), bytes: chunk_cost(offset, self.size), gated: self.gated };
            match self.client.invoke_file_part(session, &request, self.timeout, pace).await {
                Ok(base::upload::File::File(f)) => return Ok(f.bytes),
                Ok(base::upload::File::FileCdnRedirect(_)) => {
                    return Err(Error::Unexpected("upload.getFile redirected to a CDN without cdn_supported"));
                }
                Err(Error::Rpc(e)) if e.name.starts_with("FILE_REFERENCE_") && !renewed => {
                    renewed = true;
                    if !self.renew(seen).await? {
                        return Err(Error::Rpc(e));
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn renew(&self, seen: usize) -> Result<bool> {
        let _one_at_a_time = self.renewing.lock().await;
        let mut location = match &*self.location.lock().unwrap() {
            (_, renewals) if *renewals != seen => return Ok(true),
            (location, _) => location.clone(),
        };
        if !refresh(self.client, self.source, &mut location).await? {
            return Ok(false);
        }
        *self.location.lock().unwrap() = (location, seen + 1);
        Ok(true)
    }
}

impl MediaStream<'_> {
    /// Returns the next chunk, `None` at the end, or the error that stopped the download.
    pub async fn next(&mut self) -> Result<Option<Vec<u8>>> {
        poll_fn(|cx| {
            if let Some(download) = &mut self.download {
                if let Poll::Ready(done) = download.as_mut().poll(cx) {
                    self.download = None;
                    if let Err(e) = done {
                        while self.chunks.try_recv().is_ok() {}
                        return Poll::Ready(Err(e));
                    }
                }
            }
            self.chunks.poll_recv(cx).map(Ok)
        })
        .await
    }
}

// The master DC is trusted, so once the CDN fails in any way (FILE_TOKEN_INVALID, a lost key, a bad hash, a chunk
// that doesn't fit the known size) it serves the chunk that failed and every later one; chunks already written stay
// as they are.
async fn from_cdn(
    master: &Master<'_>,
    redirect: types::upload::FileCdnRedirect,
    window: &Window,
    size: Option<i64>,
    workers: usize,
    sink: &Sink,
    progress: &Progress<'_>,
) -> Result<()> {
    let client = master.client;
    let cdn = async {
        let cdn = Cdn::new(&master.sessions[0], redirect, (client.0.api_id, &client.0.options))?;
        let dc = client.dc(cdn.dc_id)?;
        // CDN chunks are neither paced nor gated, and get the connections they had before pacing whatever the
        // account: nothing is measured on a CDN, and one that floods soon hands its chunks to the master DC.
        let lease = client.cdn_sessions(dc, workers.min(MAX_CONNECTIONS)).await?;
        // A CDN part also waits on the master DC's connection for hashes, behind its chunks.
        let timeout = part_timeout(workers.div_ceil(lease.sessions.len()) as u64 * CHUNK as u64).max(master.timeout);
        Ok::<_, Error>((cdn, dc, lease, timeout))
    }
    .await
    .ok();
    let (cdn, failed) = (&cdn, &AtomicBool::new(cdn.is_none()));
    let part = move |i: usize, offset: i64| async move {
        if let (Some((cdn, dc, lease, timeout)), false) = (cdn, failed.load(Ordering::Acquire)) {
            match cdn.part(&lease.sessions[i % lease.sessions.len()], offset, CHUNK, *timeout).await {
                Ok(bytes) if fits(offset, &bytes, size).is_ok() => return Ok(bytes),
                Ok(_) => failed.store(true, Ordering::Release),
                Err(e) => {
                    if let Error::Transport(404) = e {
                        client.forget_cdn(*dc);
                    }
                    failed.store(true, Ordering::Release);
                }
            }
        }
        master.part(i, offset).await
    };
    write_all(part(0, window.first_offset()).await?, window, size, workers, sink, progress, part).await
}

async fn write_all<F: Future<Output = Result<Vec<u8>>>>(
    first: Vec<u8>,
    window: &Window,
    size: Option<i64>,
    workers: usize,
    sink: &Sink,
    progress: &Progress<'_>,
    part: impl Fn(usize, i64) -> F,
) -> Result<()> {
    let offset = window.first_offset();
    fits(offset, &first, size)?;
    let len = first.len();
    progress.add(sink.write_at(window, offset as u64, first).await?);
    let mut total = len as u64;
    if len == CHUNK as usize {
        let (next, part) = (AtomicI32::new(window.first.saturating_add(1)), &part);
        let workers = (0..workers).map(|i| chunks(&next, window, size, sink, progress, move |offset| part(i, offset)));
        total += try_join_all(workers).await?.into_iter().sum::<u64>();
    }
    if let Some(size) = size {
        let fetched =
            u64::try_from(size).ok().map(|s| (chunk(window.bound).0 as u64).min(s).saturating_sub(offset as u64));
        if fetched != Some(total) {
            return Err(Error::Unexpected("upload.getFile returned fewer bytes than the file size"));
        }
    }
    Ok(())
}

async fn chunks<F: Future<Output = Result<Vec<u8>>>>(
    next: &AtomicI32,
    window: &Window,
    size: Option<i64>,
    sink: &Sink,
    progress: &Progress<'_>,
    part: impl Fn(i64) -> F,
) -> Result<u64> {
    let mut total = 0;
    while let Some(n) = take(next, window.bound) {
        let (offset, _) = chunk(n);
        let bytes = part(offset).await?;
        fits(offset, &bytes, size)?;
        let len = bytes.len();
        progress.add(sink.write_at(window, offset as u64, bytes).await?);
        total += len as u64;
        if len < CHUNK as usize {
            break;
        }
    }
    Ok(total)
}

fn chunk_count(size: Option<i64>) -> Option<i32> {
    size.and_then(|s| u64::try_from(s).ok()).and_then(|s| i32::try_from(s.div_ceil(CHUNK as u64)).ok())
}

/// The bytes a chunk at `offset` brings, which is what it waits on a pacer for: CHUNK, less for the file's last one.
fn chunk_cost(offset: i64, size: Option<i64>) -> u64 {
    let left = size.map_or(i64::from(CHUNK), |size| size.saturating_sub(offset));
    left.clamp(0, i64::from(CHUNK)) as u64
}

// Telegram serves aligned chunks only, so a byte range is fetched as the chunks covering it and cut when written.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Window {
    start: u64,
    /// END when neither a limit nor the size bounds it.
    end: u64,
    first: i32,
    bound: i32,
}

impl Window {
    #[cfg(test)]
    const ALL: Window = Window { start: 0, end: END, first: 0, bound: i32::MAX };

    /// None when the range holds no byte, or starts past any file Telegram stores.
    fn new(offset: u64, limit: Option<u64>, size: Option<u64>) -> Option<Window> {
        let end = limit.map_or(END, |l| offset.saturating_add(l));
        let end = size.map_or(end, |s| end.min(s));
        let first = i32::try_from(offset / CHUNK as u64).ok().filter(|_| offset < end)?;
        let bound = i32::try_from(end.div_ceil(CHUNK as u64)).unwrap_or(i32::MAX);
        Some(Window { start: offset, end, first, bound })
    }

    /// 0 when unbounded.
    fn len(&self) -> u64 {
        if self.end == END { 0 } else { self.end - self.start }
    }

    fn first_offset(&self) -> i64 {
        chunk(self.first).0
    }
}

// With a known size every chunk but the last is full and the last ends at the size, so a short chunk is lost
// bytes, not the end of the file.
fn fits(offset: i64, bytes: &[u8], size: Option<i64>) -> Result<()> {
    let expected = size.map(|s| s.saturating_sub(offset).clamp(0, i64::from(CHUNK)));
    if bytes.len() > CHUNK as usize || expected.is_some_and(|e| bytes.len() as i64 != e) {
        return Err(Error::Unexpected("upload.getFile returned a chunk that doesn't match the file size"));
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
            let size = self.location.as_ref().ok().and_then(|l| u64::try_from(l.file_size?).ok());
            let capacity = Window::new(self.offset, self.limit, size).map_or(0, |w| w.len());
            let capacity = usize::try_from(capacity).unwrap_or(usize::MAX).min(MAX_PREALLOC);
            let mut out = Vec::new();
            out.try_reserve_exact(capacity).map_err(|_| out_of_memory())?;
            let sink = Sink::Memory(Mutex::new(out));
            self.download(&sink).await?;
            let Sink::Memory(out) = sink else { unreachable!() };
            Ok(out.into_inner().unwrap())
        })
    }
}

/// False when there is no message or story to re-read or it no longer holds this file.
async fn refresh(
    client: &Client,
    source: Option<DownloadSource>,
    location: &mut base::InputFileLocation,
) -> Result<bool> {
    let Some(source) = source else { return Ok(false) };
    let (file_id, reference) = match location {
        base::InputFileLocation::InputPhotoFileLocation(l) => (l.id, &mut l.file_reference),
        base::InputFileLocation::InputDocumentFileLocation(l) => (l.id, &mut l.file_reference),
        _ => return Ok(false),
    };
    let fresh = match source {
        DownloadSource::Message(chat_id, message_id) => {
            let (found, _) = client.get_raw_messages(chat_id, &[message_id]).await?;
            found.into_iter().flatten().find_map(|m| fresh_reference(m.media?, file_id))
        }
        DownloadSource::Story(chat_id, story_id) => {
            let peer = client.resolve_peer(chat_id).await?;
            let request = functions::stories::GetStoriesByID { peer, id: vec![story_id] };
            let base::stories::Stories::Stories(found) = client.invoke(&request).await?;
            found.stories.into_iter().find_map(|item| match item {
                base::StoryItem::StoryItem(s) => fresh_reference(s.media, file_id),
                _ => None,
            })
        }
    };
    let Some(fresh) = fresh else { return Ok(false) };
    *reference = fresh;
    Ok(true)
}

fn fresh_reference(media: base::MessageMedia, file_id: i64) -> Option<Vec<u8>> {
    match media {
        base::MessageMedia::MessageMediaPhoto(m) => match m.photo? {
            base::Photo::Photo(p) if p.id == file_id => Some(p.file_reference),
            _ => None,
        },
        base::MessageMedia::MessageMediaDocument(m) => match m.document? {
            base::Document::Document(d) if d.id == file_id => Some(d.file_reference),
            _ => None,
        },
        _ => None,
    }
}

enum Sink {
    Memory(Mutex<Vec<u8>>),
    File(tokio::sync::Mutex<tokio::fs::File>),
    // Each chunk waits for the ones before it, so a slow chunk holds back at most one per worker.
    Stream { sent: watch::Sender<u64>, chunks: mpsc::Sender<Vec<u8>> },
}

impl Sink {
    /// Writes the part of a chunk at `offset` that falls in `window`, placed relative to its start; returns the bytes
    /// written.
    async fn write_at(&self, window: &Window, offset: u64, mut bytes: Vec<u8>) -> Result<usize> {
        let raw = bytes.len() as u64;
        if window.end != END {
            bytes.truncate(window.end.saturating_sub(offset).min(raw) as usize);
        }
        let head = window.start.saturating_sub(offset).min(bytes.len() as u64) as usize;
        bytes.drain(..head);
        let at = offset.saturating_add(head as u64).saturating_sub(window.start);
        let written = bytes.len();
        match self {
            Sink::Memory(out) => {
                let mut out = out.lock().unwrap();
                let start = usize::try_from(at).map_err(|_| TOO_BIG)?;
                let end = start.checked_add(bytes.len()).ok_or(TOO_BIG)?;
                if out.len() < end {
                    let more = end - out.len();
                    out.try_reserve(more).map_err(|_| out_of_memory())?;
                    out.resize(end, 0);
                }
                out[start..end].copy_from_slice(&bytes);
            }
            Sink::File(file) => {
                let mut file = file.lock().await;
                file.seek(SeekFrom::Start(at)).await?;
                file.write_all(&bytes).await?;
            }
            Sink::Stream { sent, chunks } => {
                let mut turn = sent.subscribe();
                let seen = *turn.wait_for(|&s| s == offset || s == END).await.unwrap();
                if seen == END {
                    return Ok(0);
                }
                if written > 0 {
                    let _ = chunks.send(bytes).await;
                }
                let next = offset.saturating_add(raw);
                sent.send_replace(if raw < CHUNK as u64 || next >= window.end { END } else { next });
            }
        }
        Ok(written)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::io;
    use std::net::SocketAddr;
    use std::sync::atomic::AtomicI32;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use tokio::sync::{mpsc, watch};
    use tokio::time::Instant;

    use super::{CHUNK, DownloadMedia, DownloadSource, Master, MediaStream, Sink, Window, chunk, chunk_count, fits};
    use crate::Client;
    use crate::client::Transmissions;
    use crate::crypto::{aes, sha256};
    use crate::dispatcher::Entities;
    use crate::errors::{Error, Result, RpcError};
    use crate::raw::{Identifiable, Serializable, base, functions, types};
    use crate::session::Session;
    use crate::session::pacer::Lane;
    use crate::session::transfer::{take, try_join_all};
    use crate::types::messages_and_media::media::tests::document_media;
    use crate::types::{FileLocation, Message, Story};
    use crate::utils::get_peer;

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
        assert!(fits(i64::from(CHUNK), b"hi", Some(3 * i64::from(CHUNK))).is_err());
        assert!(fits(i64::from(CHUNK), b"hi", Some(i64::from(CHUNK) + 2)).is_ok());
    }

    #[tokio::test]
    async fn writes_chunks_at_their_offsets() {
        let sink = Sink::Memory(Mutex::new(Vec::new()));
        for (offset, chunk) in [(6, &b"gh"[..]), (0, b"abc"), (3, b"def")] {
            sink.write_at(&Window::ALL, offset, chunk.to_vec()).await.unwrap();
        }
        let Sink::Memory(out) = sink else { unreachable!() };
        assert_eq!(out.into_inner().unwrap(), b"abcdefgh");
    }

    #[tokio::test]
    async fn memory_that_cant_be_had_is_an_error_not_an_abort() {
        let sink = Sink::Memory(Mutex::new(b"ab".to_vec()));
        // Past isize::MAX no allocator can serve it, so the reservation fails before allocating anything.
        let out_of_memory = sink.write_at(&Window::ALL, isize::MAX as u64, b"x".to_vec()).await;
        let oom = |e: &std::io::Error| e.kind() == std::io::ErrorKind::OutOfMemory;
        assert!(matches!(&out_of_memory, Err(Error::Io(e)) if oom(e)), "{out_of_memory:?}");
        let unaddressable = sink.write_at(&Window::ALL, u64::MAX, b"x".to_vec()).await;
        assert!(matches!(unaddressable, Err(Error::Unexpected(_))), "{unaddressable:?}");
        let Sink::Memory(out) = sink else { unreachable!() };
        assert_eq!(out.into_inner().unwrap(), b"ab");
    }

    #[tokio::test]
    async fn a_memory_download_holds_exactly_its_declared_size() {
        let c = CHUNK as usize;
        let (client, _rx) = Client::scripted(vec![file(vec![1; c]), file(b"hi".into())]);
        let bytes = download(&client, Some(c as i64 + 2), None).await.0.unwrap();
        assert_eq!((bytes.len(), bytes.capacity()), (c + 2, c + 2));
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
                    sink.write_at(&Window::ALL, (n * c) as u64, vec![n as u8; len]).await
                }
            };
            // Chunk 3 is short, so it ends the file and chunk 4 is dropped.
            let parts = [arrive(2, c, 0), arrive(4, c, 1), arrive(3, 7, 2), arrive(0, c, 3), arrive(1, c, 6)];
            try_join_all(parts.into_iter()).await.map(drop)
        };
        let mut stream = MediaStream { download: Some(Box::pin(download)), chunks };
        let mut got = Vec::new();
        while let Some(chunk) = stream.next().await.unwrap() {
            got.push((chunk[0], chunk.len()));
        }
        assert_eq!(got, [(0, c), (1, c), (2, c), (3, 7)]);

        let failing = Box::pin(async { Err::<(), _>(Error::Timeout) });
        let mut failed = MediaStream { download: Some(failing), chunks: mpsc::channel(1).1 };
        assert!(matches!(failed.next().await, Err(Error::Timeout)));
        assert_eq!(failed.next().await.unwrap(), None);
    }

    fn get_file(reference: &[u8], offset: i64, cdn_supported: bool) -> functions::upload::GetFile {
        let location = types::InputDocumentFileLocation {
            id: 1,
            access_hash: 2,
            file_reference: reference.to_vec(),
            thumb_size: String::new(),
        };
        let location = base::InputFileLocation::InputDocumentFileLocation(Box::new(location));
        functions::upload::GetFile { precise: false, cdn_supported, location, offset, limit: CHUNK }
    }

    fn file(bytes: Vec<u8>) -> Result<Vec<u8>> {
        let file = types::upload::File { r#type: base::storage::FileType::FilePartial, mtime: 0, bytes };
        Ok(base::upload::File::File(Box::new(file)).to_bytes())
    }

    fn error(name: &str) -> Result<Vec<u8>> {
        Err(Error::Rpc(RpcError { code: 400, name: name.into(), value: None }))
    }

    /// Message 5 in group -5, holding document 1 under reference [3].
    fn messages() -> Result<Vec<u8>> {
        let mut message = Message::bare(5, get_peer(-5), 0);
        message.media = Some(document_media("text/plain", Vec::new()));
        let messages = types::messages::Messages {
            messages: vec![base::Message::Message(Box::new(message))],
            topics: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        Ok(base::messages::Messages::Messages(Box::new(messages)).to_bytes())
    }

    /// Downloads document 1 under reference "old"; returns the bytes or the error name, and the progress reported.
    async fn download(
        client: &Client,
        size: Option<i64>,
        source: Option<DownloadSource>,
    ) -> (Result<Vec<u8>, String>, Vec<u64>) {
        client.0.me.lock().unwrap().premium = Some(false);
        // Two workers when the size spans chunks; both use the scripted session, so nothing goes to the network.
        client.0.transfers.insert(client.dc(2).unwrap(), &client.session());
        let location = FileLocation { location: get_file(b"old", 0, true).location, dc_id: Some(2), file_size: size };
        let done = Arc::new(Mutex::new(Vec::new()));
        let log = done.clone();
        let download = DownloadMedia::new(client, Ok(location), source);
        let result = download.progress(move |d, _| log.lock().unwrap().push(d)).await.map_err(|e| match e {
            Error::Rpc(e) => e.name,
            e => e.to_string(),
        });
        let done = done.lock().unwrap().clone();
        (result, done)
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vaegram-download-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn leftovers(dir: &std::path::Path) -> Vec<String> {
        let mut names: Vec<_> =
            std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    fn to_file<'a>(client: &'a Client, location: Result<FileLocation>) -> DownloadMedia<'a> {
        client.0.me.lock().unwrap().premium = Some(false);
        DownloadMedia::new(client, location, None)
    }

    #[tokio::test]
    async fn a_bad_file_id_leaves_an_existing_file_alone() {
        let dir = scratch("bad-id");
        let path = dir.join("keep.jpg");
        std::fs::write(&path, b"old").unwrap();
        let (client, _rx) = Client::scripted(Vec::new());
        let result = client.download_media("not a file id").save(&path).await;
        assert!(result.is_err());
        assert_eq!((std::fs::read(&path).unwrap(), leftovers(&dir)), (b"old".to_vec(), vec!["keep.jpg".to_string()]));
        let fresh = dir.join("sub").join("new.jpg");
        assert!(client.download_media("not a file id").save(&fresh).await.is_err());
        assert!(!dir.join("sub").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn a_failed_download_keeps_the_old_file() {
        let dir = scratch("failed");
        let path = dir.join("keep.bin");
        std::fs::write(&path, b"old").unwrap();
        let (client, _rx) = Client::scripted(vec![error("FILE_REFERENCE_EXPIRED")]);
        client.0.transfers.insert(client.dc(2).unwrap(), &client.session());
        let location =
            FileLocation { location: get_file(b"old", 0, true).location, dc_id: Some(2), file_size: Some(2) };
        assert!(to_file(&client, Ok(location)).save(&path).await.is_err());
        assert_eq!((std::fs::read(&path).unwrap(), leftovers(&dir)), (b"old".to_vec(), vec!["keep.bin".to_string()]));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn a_successful_download_replaces_the_file() {
        let dir = scratch("replaced");
        let path = dir.join("out.bin");
        std::fs::write(&path, b"old").unwrap();
        let (client, _rx) = Client::scripted(vec![file(b"hi".into())]);
        client.0.transfers.insert(client.dc(2).unwrap(), &client.session());
        let location =
            FileLocation { location: get_file(b"old", 0, true).location, dc_id: Some(2), file_size: Some(2) };
        to_file(&client, Ok(location)).save(&path).await.unwrap();
        assert_eq!((std::fs::read(&path).unwrap(), leftovers(&dir)), (b"hi".to_vec(), vec!["out.bin".to_string()]));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn a_cancelled_download_leaves_no_file() {
        let dir = scratch("cancelled");
        let path = dir.join("out.bin");
        let (session, mut requests) = Session::silent();
        let client = Client::with_session(session);
        let location =
            FileLocation { location: get_file(b"old", 0, true).location, dc_id: Some(2), file_size: Some(2) };
        let mut download = Box::pin(to_file(&client, Ok(location)).save(&path));
        // The request is out and unanswered: the download is mid-flight, with its temporary file on disk.
        tokio::select! {
            _ = &mut download => panic!("nothing answered"),
            _ = requests.recv() => {}
        }
        let names = leftovers(&dir);
        assert!(matches!(&names[..], [name] if name.starts_with("out.bin.") && name.ends_with(".part")), "{names:?}");
        drop(download);
        assert_eq!(leftovers(&dir), Vec::<String>::new());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn a_directory_is_not_a_destination() {
        let dir = scratch("directory");
        let (client, _rx) = Client::scripted(Vec::new());
        let location =
            FileLocation { location: get_file(b"old", 0, true).location, dc_id: Some(2), file_size: Some(2) };
        assert!(matches!(to_file(&client, Ok(location)).save(&dir).await, Err(Error::InvalidArgument(_))));
        assert_eq!(leftovers(&dir), Vec::<String>::new());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn drain(rx: &mut mpsc::UnboundedReceiver<Vec<u8>>) -> Vec<Vec<u8>> {
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    #[cfg(feature = "websocket")]
    #[tokio::test]
    async fn websocket_turns_off_cdn() {
        let (mut client, mut rx) = Client::scripted(vec![file(b"hi".into())]);
        Arc::get_mut(&mut client.0).unwrap().options.websocket = true;
        let (result, _) = download(&client, Some(2), None).await;
        assert_eq!((result, drain(&mut rx)), (Ok(b"hi".to_vec()), vec![get_file(b"old", 0, false).to_bytes()]));
    }

    #[tokio::test]
    async fn refetches_the_message_once_for_an_expired_reference() {
        let get_messages = functions::messages::GetMessages::CONSTRUCTOR_ID.to_le_bytes();
        let (old, fresh) = (get_file(b"old", 0, true).to_bytes(), get_file(&[3], 0, true).to_bytes());
        let expired = || Err("FILE_REFERENCE_EXPIRED".into());
        let source = Some(DownloadSource::Message(-5, 5));

        let (client, mut rx) = Client::scripted(vec![error("FILE_REFERENCE_EXPIRED"), messages(), file(b"hi".into())]);
        let (result, _) = download(&client, Some(2), source).await;
        let sent = drain(&mut rx);
        assert_eq!((result, [&sent[0], &sent[2]]), (Ok(b"hi".to_vec()), [&old, &fresh]));
        assert_eq!(sent[1][..4], get_messages);

        let replies = vec![error("FILE_REFERENCE_EXPIRED"), messages(), error("FILE_REFERENCE_EXPIRED")];
        let (client, mut rx) = Client::scripted(replies);
        assert_eq!((download(&client, Some(5), source).await.0, drain(&mut rx).len()), (expired(), 3));
        let (client, mut rx) = Client::scripted(vec![error("FILE_REFERENCE_EXPIRED")]);
        assert_eq!((download(&client, Some(5), None).await.0, drain(&mut rx)), (expired(), vec![old]));
    }

    /// Story 7, holding document 1 under reference [3].
    fn story_item() -> types::StoryItem {
        types::StoryItem {
            pinned: false,
            public: false,
            close_friends: false,
            min: false,
            noforwards: false,
            edited: false,
            contacts: false,
            selected_contacts: false,
            out: false,
            id: 7,
            date: 0,
            from_id: None,
            fwd_from: None,
            expire_date: 0,
            caption: None,
            entities: None,
            media: document_media("text/plain", Vec::new()),
            media_areas: None,
            privacy: None,
            views: None,
            sent_reaction: None,
            albums: None,
            music: None,
        }
    }

    fn stories() -> Result<Vec<u8>> {
        let stories = types::stories::Stories {
            count: 1,
            stories: vec![base::StoryItem::StoryItem(Box::new(story_item()))],
            pinned_to_top: None,
            chats: Vec::new(),
            users: Vec::new(),
        };
        Ok(base::stories::Stories::Stories(Box::new(stories)).to_bytes())
    }

    #[tokio::test]
    async fn refetches_the_story_once_for_an_expired_reference() {
        let (old, fresh) = (get_file(b"old", 0, true).to_bytes(), get_file(&[3], 0, true).to_bytes());
        let (client, mut rx) = Client::scripted(vec![error("FILE_REFERENCE_EXPIRED"), stories(), file(b"hi".into())]);
        let (result, _) = download(&client, Some(2), Some(DownloadSource::Story(-5, 7))).await;
        let sent = drain(&mut rx);
        assert_eq!((result, [&sent[0], &sent[2]]), (Ok(b"hi".to_vec()), [&old, &fresh]));
        let peer = base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 5 });
        assert_eq!(sent[1], functions::stories::GetStoriesByID { peer, id: vec![7] }.to_bytes());
    }

    #[tokio::test]
    async fn a_story_download_renews_through_the_story() {
        let (client, mut rx) =
            Client::scripted(vec![error("FILE_REFERENCE_EXPIRED"), stories(), file(b"hello".into())]);
        client.0.me.lock().unwrap().premium = Some(false);
        let item = base::StoryItem::StoryItem(Box::new(story_item()));
        let story = Story::parse(&client, &get_peer(-5), item, &Entities::default()).unwrap();
        assert_eq!(story.download().await.unwrap(), b"hello");
        let peer = base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 5 });
        assert_eq!(drain(&mut rx)[1], functions::stories::GetStoriesByID { peer, id: vec![7] }.to_bytes());
    }

    #[tokio::test]
    async fn renews_a_reference_that_expires_mid_file() {
        let c = CHUNK as usize;
        let replies = vec![file(vec![1; c]), error("FILE_REFERENCE_EXPIRED"), messages(), file(b"hi".into())];
        let (client, mut rx) = Client::scripted(replies);
        let (result, done) = download(&client, None, Some(DownloadSource::Message(-5, 5))).await;
        let sent = drain(&mut rx);
        let expected = [vec![1; c], b"hi".to_vec()].concat();
        assert_eq!((result, done), (Ok(expected), vec![c as u64, c as u64 + 2]));
        let (old, fresh) = (get_file(b"old", i64::from(CHUNK), false), get_file(&[3], i64::from(CHUNK), false));
        assert_eq!((sent.len(), &sent[1], &sent[3]), (4, &old.to_bytes(), &fresh.to_bytes()));
    }

    #[tokio::test]
    async fn workers_hitting_the_same_expiry_renew_it_once() {
        let expired = || error("FILE_REFERENCE_EXPIRED");
        let replies = vec![expired(), expired(), messages(), file(b"a".into()), file(b"b".into())];
        let (client, mut rx) = Client::scripted(replies);
        let sessions = [client.session()];
        let master = Master {
            timeout: crate::session::transfer::MIN_PART_TIMEOUT,
            client: &client,
            sessions: &sessions,
            source: Some(DownloadSource::Message(-5, 5)),
            location: Mutex::new((get_file(b"old", 0, false).location, 0)),
            renewing: tokio::sync::Mutex::new(()),
            pacer: None,
            gated: false,
            size: None,
        };
        let parts = try_join_all([master.part(0, 0), master.part(1, i64::from(CHUNK))].into_iter()).await.unwrap();
        let get_messages = functions::messages::GetMessages::CONSTRUCTOR_ID.to_le_bytes();
        let sent = drain(&mut rx);
        let renewals = sent.iter().filter(|b| b[..4] == get_messages).count();
        assert_eq!((parts, renewals, sent.len()), (vec![b"a".to_vec(), b"b".to_vec()], 1, 5));
        assert_eq!(master.location.lock().unwrap().1, 1);
    }

    /// Chunk 0 comes from the CDN, chunk 1 gets `second` from it and `master` from the master DC if it falls back;
    /// `next_hashes` answers the CDN check's question about a hash after a short part. Returns the requests the master
    /// DC and the CDN got.
    async fn via_cdn(
        second: Result<Vec<u8>>,
        next_hashes: Option<Vec<base::FileHash>>,
        master: Vec<u8>,
        size: Option<i64>,
    ) -> (Vec<Vec<u8>>, usize) {
        let c = CHUNK as usize;
        let (key, iv) = ([1u8; 32], [2u8; 16]);
        let plain: Vec<u8> = (0..c).map(|i| (i * 7) as u8).collect();
        let (mut cipher, mut counter) = (plain.clone(), iv);
        counter[12..].fill(0);
        aes::ctr256(&mut cipher, &key, &mut counter, &mut 0);
        let hash = types::FileHash { offset: 0, limit: CHUNK, hash: sha256(&[&plain]).to_vec() };
        let redirect = types::upload::FileCdnRedirect {
            dc_id: 203,
            file_token: b"token".to_vec(),
            encryption_key: key.to_vec(),
            encryption_iv: iv.to_vec(),
            file_hashes: vec![base::FileHash::FileHash(Box::new(hash))],
        };
        let redirect = Ok(base::upload::File::FileCdnRedirect(Box::new(redirect)).to_bytes());
        let part = base::upload::CdnFile::CdnFile(Box::new(types::upload::CdnFile { bytes: cipher }));
        let (cdn, mut cdn_rx) = Session::scripted(vec![Ok(part.to_bytes()), second]);

        let mut replies = vec![redirect];
        replies.extend(next_hashes.map(|h| Ok(h.to_bytes())));
        replies.push(file(master.clone()));
        let (client, mut rx) = Client::scripted(replies);
        client.serve_cdn(203, &cdn);
        client.0.transfers.insert(client.dc(203).unwrap(), &cdn);
        let (result, done) = download(&client, size, None).await;
        let total = (c + master.len()) as u64;
        assert_eq!((result, done), (Ok([plain, master].concat()), vec![c as u64, total]));
        (drain(&mut rx), drain(&mut cdn_rx).len())
    }

    #[tokio::test]
    async fn the_master_dc_serves_the_rest_once_the_cdn_fails() {
        let (sent, cdn_sent) = via_cdn(error("FILE_TOKEN_INVALID"), None, b"hi".to_vec(), None).await;
        let master = [get_file(b"old", 0, true).to_bytes(), get_file(b"old", i64::from(CHUNK), false).to_bytes()];
        assert_eq!((sent, cdn_sent), (master.to_vec(), 2));
    }

    #[tokio::test]
    async fn the_master_dc_serves_a_chunk_the_cdn_cut_short() {
        let c = CHUNK as usize;
        let empty = base::upload::CdnFile::CdnFile(Box::new(types::upload::CdnFile { bytes: Vec::new() })).to_bytes();
        let more = types::FileHash { offset: i64::from(CHUNK), limit: 2, hash: vec![0; 32] };
        let more = || Some(vec![base::FileHash::FileHash(Box::new(more.clone()))]);
        // Without a size only the master's hash after the cut shows the file goes on.
        for (master, size) in [(vec![9; c], Some(2 * c as i64)), (b"hi".to_vec(), None)] {
            let (sent, cdn_sent) = via_cdn(Ok(empty.clone()), more(), master, size).await;
            assert_eq!((sent.len(), cdn_sent), (3, 2));
        }
    }

    #[tokio::test]
    async fn a_short_chunk_before_the_known_end_is_an_error() {
        let c = CHUNK as usize;
        let short = |result: Result<Vec<u8>, String>| result.is_err_and(|e| e.contains("doesn't match the file size"));
        let (client, _rx) = Client::scripted(vec![file(vec![1; c]), file(b"hi".into())]);
        assert!(short(download(&client, Some(2 * c as i64), None).await.0));
        let (client, _rx) = Client::scripted(vec![file(b"hi".into())]);
        assert!(short(download(&client, Some(5), None).await.0));
    }

    #[tokio::test]
    async fn a_takeout_download_wraps_every_chunk_and_skips_the_cdn() {
        let c = CHUNK as usize;
        let (client, mut rx) = Client::scripted(vec![file(vec![1; c]), file(b"hi".into())]);
        let takeout = Client(client.0.clone(), Some(7));
        let (result, _) = download(&takeout, None, None).await;
        assert_eq!(result, Ok([vec![1; c], b"hi".to_vec()].concat()));
        let wrapped = |offset| functions::InvokeWithTakeout { takeout_id: 7, query: get_file(b"old", offset, false) };
        assert_eq!(drain(&mut rx), [wrapped(0).to_bytes(), wrapped(i64::from(CHUNK)).to_bytes()]);
    }

    /// `n` chunks whose bytes differ from one offset to the next.
    fn numbered(n: usize) -> Vec<u8> {
        (0..n * CHUNK as usize).map(|i| (i % 251) as u8).collect()
    }

    fn location(size: Option<i64>) -> FileLocation {
        FileLocation { location: get_file(b"old", 0, true).location, dc_id: Some(2), file_size: size }
    }

    fn located(client: &Client, size: Option<i64>) -> FileLocation {
        client.0.me.lock().unwrap().premium = Some(false);
        client.0.transfers.insert(client.dc(2).unwrap(), &client.session());
        location(size)
    }

    /// The bytes or the error, and each `(done, total)` reported.
    type Ranged = (Result<Vec<u8>, String>, Vec<(u64, u64)>);

    /// Bytes `offset..offset + limit` of document 1, to the end without a limit.
    async fn ranged(
        client: &Client,
        size: Option<i64>,
        offset: u64,
        limit: Option<u64>,
        source: Option<DownloadSource>,
    ) -> Ranged {
        let mut download = DownloadMedia::new(client, Ok(located(client, size)), source).offset(offset);
        if let Some(limit) = limit {
            download = download.limit(limit);
        }
        let done = Arc::new(Mutex::new(Vec::new()));
        let log = done.clone();
        let result = download.progress(move |d, t| log.lock().unwrap().push((d, t))).await.map_err(|e| e.to_string());
        let done = done.lock().unwrap().clone();
        (result, done)
    }

    /// Bytes CHUNK + 10 .. 2 * CHUNK + 10 of a three-chunk file.
    async fn middle_of_three() -> (Ranged, Vec<Vec<u8>>, Vec<u8>) {
        let (c, whole) = (CHUNK as usize, numbered(3));
        let (client, mut rx) = Client::scripted(vec![file(whole[c..2 * c].to_vec()), file(whole[2 * c..].to_vec())]);
        let got = ranged(&client, Some(3 * c as i64), c as u64 + 10, Some(c as u64), None).await;
        (got, drain(&mut rx), whole)
    }

    #[tokio::test]
    async fn a_range_asks_only_for_the_chunks_it_covers() {
        let c = CHUNK as usize;
        let ((result, _), sent, whole) = middle_of_three().await;
        assert_eq!(result, Ok(whole[c + 10..2 * c + 10].to_vec()));
        let at = i64::from(CHUNK);
        assert_eq!(sent, [get_file(b"old", at, true).to_bytes(), get_file(b"old", 2 * at, false).to_bytes()]);
    }

    #[tokio::test]
    async fn progress_counts_only_the_range() {
        let c = CHUNK as u64;
        let ((_, done), _, _) = middle_of_three().await;
        assert_eq!(done, [(c - 10, c), (c, c)]);
    }

    #[tokio::test]
    async fn a_range_stream_yields_only_the_range_in_order() {
        let (c, whole) = (CHUNK as usize, numbered(3));
        let (client, _rx) = Client::scripted(vec![file(whole[c..2 * c].to_vec()), file(whole[2 * c..].to_vec())]);
        let mut stream = client.download_media(located(&client, Some(3 * c as i64))).offset(c as u64 + 10).stream();
        let mut got = Vec::new();
        while let Some(piece) = stream.next().await.unwrap() {
            got.push(piece);
        }
        assert_eq!(got, [whole[c + 10..2 * c].to_vec(), whole[2 * c..].to_vec()]);
    }

    #[tokio::test]
    async fn a_range_to_a_file_starts_at_the_files_start() {
        let whole = numbered(1);
        let (client, _rx) = Client::scripted(vec![file(whole.clone())]);
        let location = located(&client, Some(2 * i64::from(CHUNK)));
        let path = std::env::temp_dir().join(format!("vaegram-range-{}.bin", std::process::id()));
        client.download_media(location).offset(5).limit(10).save(&path).await.unwrap();
        let written = std::fs::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(written, whole[5..15]);
    }

    #[tokio::test]
    async fn an_offset_past_the_known_end_sends_nothing() {
        let (client, mut rx) = Client::recording();
        assert_eq!(client.download_media(location(Some(5))).offset(5).await.unwrap(), b"");
        assert_eq!(client.download_media(location(Some(5))).offset(9).stream().next().await.unwrap(), None);
        // No file Telegram stores reaches this chunk, whatever size it gives.
        assert_eq!(client.download_media(location(None)).offset(u64::MAX).await.unwrap(), b"");
        assert!(drain(&mut rx).is_empty());
    }

    #[tokio::test]
    async fn limit_zero_sends_nothing() {
        let (client, mut rx) = Client::recording();
        assert_eq!(client.download_media(location(None)).limit(0).await.unwrap(), b"");
        assert_eq!(client.download_media(location(Some(5))).offset(2).limit(0).await.unwrap(), b"");
        assert!(drain(&mut rx).is_empty());
    }

    #[tokio::test]
    async fn an_open_range_without_a_size_ends_at_the_short_chunk() {
        let (c, at) = (CHUNK as usize, i64::from(CHUNK));
        let (client, mut rx) = Client::scripted(vec![file(vec![1; c]), file(b"hi".into())]);
        let (result, _) = ranged(&client, None, c as u64 + 3, None, None).await;
        assert_eq!(result, Ok([vec![1; c - 3], b"hi".to_vec()].concat()));
        assert_eq!(drain(&mut rx), [get_file(b"old", at, true).to_bytes(), get_file(b"old", 2 * at, false).to_bytes()]);
    }

    #[tokio::test]
    async fn a_ranged_takeout_download_wraps_every_chunk_and_skips_the_cdn() {
        let c = CHUNK as usize;
        let (client, mut rx) = Client::scripted(vec![file(vec![1; c]), file(b"hi".into())]);
        let takeout = Client(client.0.clone(), Some(7));
        let (result, _) = ranged(&takeout, None, c as u64 + 1, None, None).await;
        assert_eq!(result, Ok([vec![1; c - 1], b"hi".to_vec()].concat()));
        let wrapped = |offset| functions::InvokeWithTakeout { takeout_id: 7, query: get_file(b"old", offset, false) };
        let at = i64::from(CHUNK);
        assert_eq!(drain(&mut rx), [wrapped(at).to_bytes(), wrapped(2 * at).to_bytes()]);
    }

    #[tokio::test]
    async fn a_ranged_download_renews_an_expired_reference() {
        let at = i64::from(CHUNK);
        let replies = vec![error("FILE_REFERENCE_EXPIRED"), messages(), file(b"hello".into())];
        let (client, mut rx) = Client::scripted(replies);
        let (result, _) = ranged(&client, None, at as u64 + 1, None, Some(DownloadSource::Message(-5, 5))).await;
        assert_eq!(result, Ok(b"ello".to_vec()));
        let sent = drain(&mut rx);
        let (old, fresh) = (get_file(b"old", at, true).to_bytes(), get_file(&[3], at, true).to_bytes());
        assert_eq!((sent.len(), &sent[0], &sent[2]), (3, &old, &fresh));
    }

    #[tokio::test]
    async fn a_cdn_range_fetches_the_missing_hashes() {
        let at = i64::from(CHUNK);
        let (key, iv) = ([1u8; 32], [2u8; 16]);
        let plain: Vec<u8> = (0..100).map(|i| (i * 7) as u8).collect();
        let (mut cipher, mut counter) = (plain.clone(), iv);
        counter[12..].copy_from_slice(&((at / 16) as u32).to_be_bytes());
        aes::ctr256(&mut cipher, &key, &mut counter, &mut 0);
        let hash = |offset, bytes: &[u8]| {
            let hash = types::FileHash { offset, limit: bytes.len() as i32, hash: sha256(&[bytes]).to_vec() };
            base::FileHash::FileHash(Box::new(hash))
        };
        // The redirect only covers the start of the file.
        let redirect = types::upload::FileCdnRedirect {
            dc_id: 203,
            file_token: b"token".to_vec(),
            encryption_key: key.to_vec(),
            encryption_iv: iv.to_vec(),
            file_hashes: vec![hash(0, &[0; 16])],
        };
        let redirect = Ok(base::upload::File::FileCdnRedirect(Box::new(redirect)).to_bytes());
        let part = base::upload::CdnFile::CdnFile(Box::new(types::upload::CdnFile { bytes: cipher }));
        let (cdn, mut cdn_rx) = Session::scripted(vec![Ok(part.to_bytes())]);
        let none: Vec<base::FileHash> = Vec::new();
        let replies = vec![redirect, Ok(vec![hash(at, &plain)].to_bytes()), Ok(none.to_bytes())];
        let (client, mut rx) = Client::scripted(replies);
        client.serve_cdn(203, &cdn);
        client.0.transfers.insert(client.dc(203).unwrap(), &cdn);

        let (result, _) = ranged(&client, Some(at + 100), at as u64 + 3, None, None).await;
        assert_eq!(result, Ok(plain[3..].to_vec()));
        let hashes = |offset| functions::upload::GetCdnFileHashes { file_token: b"token".to_vec(), offset }.to_bytes();
        assert_eq!(drain(&mut rx), [get_file(b"old", at, true).to_bytes(), hashes(at), hashes(at + 100)]);
        assert_eq!(drain(&mut cdn_rx).len(), 1);
    }

    fn one_download_at_a_time(client: &mut Client) {
        Arc::get_mut(&mut client.0).unwrap().transmissions = Some(Transmissions::new(1));
    }

    #[tokio::test]
    async fn a_second_download_waits_for_the_first() {
        let (mut client, mut rx) = Client::scripted(vec![file(b"hi".to_vec())]);
        one_download_at_a_time(&mut client);
        let location = located(&client, Some(2));
        let held = client.download_slot().await;
        let second = tokio::spawn({
            let client = client.clone();
            async move { client.download_media(location).await }
        });
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }
        assert!(!second.is_finished(), "the download ran without a slot");
        assert!(drain(&mut rx).is_empty());
        drop(held);
        assert_eq!(second.await.unwrap().unwrap(), b"hi");
    }

    #[tokio::test]
    async fn an_empty_window_takes_no_permit() {
        let (mut client, mut rx) = Client::recording();
        one_download_at_a_time(&mut client);
        let _held = client.download_slot().await;
        let empty = client.download_media(location(Some(5))).offset(5).await;
        assert_eq!(empty.unwrap(), b"");
        assert!(drain(&mut rx).is_empty());
    }

    #[tokio::test]
    async fn a_dropped_stream_frees_its_permit() {
        // Two chunks, so the download uses two connections: the client's own session, registered twice (by
        // `with_session` and `located`). A third chunk would make it ask for a third connection, and the pool would
        // dial the real data center, so the test would pass or fail with the network.
        let (c, whole) = (CHUNK as usize, numbered(2));
        let replies = (0..2).map(|i| file(whole[i * c..(i + 1) * c].to_vec())).collect();
        let (mut client, _rx) = Client::scripted(replies);
        one_download_at_a_time(&mut client);
        let slots = client.0.transmissions.as_ref().unwrap().downloads.clone();
        let mut stream = client.download_media(located(&client, Some(2 * c as i64))).stream();
        assert!(stream.next().await.unwrap().is_some());
        assert_eq!(slots.available_permits(), 0, "the stream isn't done, so it keeps its slot");
        drop(stream);
        assert_eq!(slots.available_permits(), 1);
    }

    /// A chunk's time at a bot's download pace of 20 MiB/s: 25 ms.
    fn chunk_time() -> Duration {
        Duration::from_nanos(CHUNK as u64 * 1_000_000_000 / (20 << 20))
    }

    /// Document 1, `size` bytes on DC `dc_id`.
    fn document(dc_id: i32, size: usize) -> FileLocation {
        FileLocation { location: get_file(b"old", 0, true).location, dc_id: Some(dc_id), file_size: Some(size as i64) }
    }

    /// [`document`], with 8 connections to DC `dc_id`: the client's session each time, so they share one gate.
    fn on_eight(client: &Client, dc_id: i32, size: usize) -> FileLocation {
        for _ in 0..8 {
            client.0.transfers.insert(client.dc(dc_id).unwrap(), &client.session());
        }
        document(dc_id, size)
    }

    type Log = mpsc::UnboundedReceiver<(Instant, usize)>;

    /// `n` connections to DC 2 in `client`'s pool, each answering with a full chunk a second after it's asked.
    fn slow_dc(client: &Client, n: usize) -> Vec<(Session, Log)> {
        let slow = || {
            let (session, log) = Session::slow(Duration::from_secs(1), || file(vec![1; CHUNK as usize]));
            client.0.transfers.insert(client.dc(2).unwrap(), &session);
            (session, log)
        };
        (0..n).map(|_| slow()).collect()
    }

    /// Every request the connections took, by when it came: which connection, how long after `start`, and how many
    /// that connection had unanswered then, that one included.
    fn requests(dc: &mut [(Session, Log)], start: Instant) -> Vec<(usize, Duration, usize)> {
        let mut requests = Vec::new();
        for (i, (_, log)) in dc.iter_mut().enumerate() {
            while let Ok((at, unanswered)) = log.try_recv() {
                requests.push((i, at - start, unanswered));
            }
        }
        requests.sort_by_key(|&(i, at, _)| (at, i));
        requests
    }

    fn as_user(client: &Client) {
        let mut me = client.0.me.lock().unwrap();
        (me.is_bot, me.premium) = (false, Some(false));
    }

    /// How long after `start` each of the first `n` requests reached the session behind `rx`.
    async fn sent_after(rx: &mut mpsc::UnboundedReceiver<Vec<u8>>, start: Instant, n: usize) -> Vec<Duration> {
        let mut after = Vec::new();
        while after.len() < n && rx.recv().await.is_some() {
            after.push(start.elapsed());
        }
        after
    }

    /// Asserts the k-th request went in its turn, k chunks' time in, give or take the timer's millisecond.
    fn in_turn(after: &[Duration]) {
        for (k, at) in after.iter().enumerate() {
            let turn = chunk_time() * k as u32;
            assert!(turn <= *at && *at <= turn + Duration::from_millis(1), "request {k} went at {at:?}, not {turn:?}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_bots_chunks_go_one_at_a_time_at_its_pace() {
        let c = CHUNK as usize;
        let (client, mut rx) = Client::scripted((0..16).map(|_| file(vec![1; c])).collect());
        let (start, location) = (Instant::now(), on_eight(&client, 2, 16 * c));
        let download = async { client.download_media(location).await };
        let (bytes, after) = tokio::join!(download, sent_after(&mut rx, start, 16));
        assert_eq!(bytes.unwrap().len(), 16 * c);
        in_turn(&after);
    }

    #[tokio::test(start_paused = true)]
    async fn a_bot_keeps_one_chunk_in_flight_on_each_of_8_connections_from_8_mib() {
        let (client, c) = (Client::offline(), CHUNK as usize);
        let mut dc = slow_dc(&client, 8);
        let start = Instant::now();
        assert_eq!(client.download_media(document(2, 16 * c)).await.unwrap().len(), 16 * c);
        let requests = requests(&mut dc, start);
        assert!(requests.iter().all(|r| r.2 == 1), "{requests:?}");
        // The first chunk alone; once it's in, a second later, one on each connection in turn.
        assert_eq!((requests.len(), requests[0].1), (16, Duration::ZERO));
        for (k, &(_, at, _)) in requests[1..9].iter().enumerate() {
            let turn = Duration::from_secs(1) + chunk_time() * k as u32;
            assert!(turn <= at && at <= turn + Duration::from_millis(1), "request {k} went at {at:?}, not {turn:?}");
        }
        assert_eq!(requests[1..9].iter().map(|r| r.0).collect::<HashSet<_>>().len(), 8);
    }

    #[tokio::test(start_paused = true)]
    async fn a_user_account_keeps_4_chunks_in_flight_unpaced() {
        let (client, c) = (Client::offline(), CHUNK as usize);
        as_user(&client);
        let mut dc = slow_dc(&client, 8);
        let start = Instant::now();
        assert_eq!(client.download_media(document(2, 16 * c)).await.unwrap().len(), 16 * c);
        let requests = requests(&mut dc, start);
        let second = Duration::from_secs(1);
        let first: Vec<_> = requests[..5].iter().map(|r| r.1).collect();
        assert_eq!(first, [Duration::ZERO, second, second, second, second]);
        assert_eq!(requests.iter().map(|r| r.0).collect::<HashSet<_>>().len(), 4);
    }

    #[tokio::test(start_paused = true)]
    async fn a_bots_download_under_8_mib_goes_as_a_users() {
        let c = CHUNK as usize;
        for bot in [true, false] {
            let client = Client::offline();
            if !bot {
                as_user(&client);
            }
            let mut dc = slow_dc(&client, 8);
            let start = Instant::now();
            assert_eq!(client.download_media(document(2, 6 * c)).await.unwrap().len(), 6 * c);
            let requests = requests(&mut dc, start);
            // The first chunk alone, 4 at once on 4 connections when it's in, and the last when one of those is.
            let second = Duration::from_secs(1);
            let at: Vec<_> = requests.iter().map(|r| r.1).collect();
            assert_eq!(at, [Duration::ZERO, second, second, second, second, 2 * second], "bot: {bot}");
            assert_eq!(requests.iter().map(|r| r.0).collect::<HashSet<_>>().len(), 4, "bot: {bot}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn many_small_downloads_by_a_bot_at_once_all_go_at_once() {
        let small = 200 << 10;
        let (client, mut rx) = Client::scripted((0..50).map(|_| file(vec![1; small])).collect());
        on_eight(&client, 2, small);
        let start = Instant::now();
        let client = &client;
        let download = |_| async move { client.download_media(document(2, small)).await };
        let (bytes, after) = tokio::join!(try_join_all((0..50).map(download)), sent_after(&mut rx, start, 50));
        assert!(bytes.unwrap().iter().all(|b| b.len() == small));
        assert_eq!(after, [Duration::ZERO; 50]);
    }

    #[tokio::test(start_paused = true)]
    async fn two_bot_downloads_from_one_dc_keep_one_getfile_on_each_connection() {
        let (client, c) = (Client::offline(), CHUNK as usize);
        let mut dc = slow_dc(&client, 8);
        let start = Instant::now();
        let (one, two) = (document(2, 20 * c), document(2, 20 * c));
        let (one, two) = (async { client.download_media(one).await }, async { client.download_media(two).await });
        let (one, two) = tokio::join!(one, two);
        assert_eq!((one.unwrap().len(), two.unwrap().len()), (20 * c, 20 * c));
        let requests = requests(&mut dc, start);
        assert_eq!(requests.len(), 40);
        assert!(requests.iter().all(|r| r.2 == 1), "{requests:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn a_lease_short_of_connections_keeps_one_getfile_on_each() {
        let (mut client, c) = (Client::offline(), CHUNK as usize);
        // Every connection the lease would add fails to open, so it goes on with the 6 in the pool.
        let refuse = |_: SocketAddr| async {
            Err::<tokio::io::DuplexStream, _>(io::Error::new(io::ErrorKind::ConnectionRefused, "no route"))
        };
        Arc::get_mut(&mut client.0).unwrap().options.connection_factory = Some(Arc::new(refuse));
        let mut dc = slow_dc(&client, 6);
        let start = Instant::now();
        assert_eq!(client.download_media(document(2, 20 * c)).await.unwrap().len(), 20 * c);
        let requests = requests(&mut dc, start);
        assert_eq!(requests.iter().map(|r| r.0).collect::<HashSet<_>>().len(), 6);
        assert!(requests.iter().all(|r| r.2 == 1), "{requests:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn downloads_from_one_dc_share_its_pace() {
        let c = CHUNK as usize;
        let (client, mut rx) = Client::scripted((0..32).map(|_| file(vec![1; c])).collect());
        let (start, one, two) = (Instant::now(), on_eight(&client, 2, 16 * c), on_eight(&client, 2, 16 * c));
        let (one, two) = (async { client.download_media(one).await }, async { client.download_media(two).await });
        let (one, two, after) = tokio::join!(one, two, sent_after(&mut rx, start, 32));
        assert_eq!((one.unwrap().len(), two.unwrap().len()), (16 * c, 16 * c));
        in_turn(&after);
    }

    #[tokio::test(start_paused = true)]
    async fn downloads_from_two_dcs_are_paced_apart() {
        let c = CHUNK as usize;
        let (client, mut rx) = Client::scripted((0..32).map(|_| file(vec![1; c])).collect());
        let (start, two, four) = (Instant::now(), on_eight(&client, 2, 16 * c), on_eight(&client, 4, 16 * c));
        let (two, four) = (async { client.download_media(two).await }, async { client.download_media(four).await });
        let (two, four, after) = tokio::join!(two, four, sent_after(&mut rx, start, 2));
        assert!(two.is_ok() && four.is_ok());
        // Each DC's first chunk goes at once.
        assert_eq!(after, [Duration::ZERO; 2]);
    }

    #[tokio::test(start_paused = true)]
    async fn small_files_are_not_held_back() {
        let small = 100 << 10;
        let (client, mut rx) = Client::scripted((0..3).map(|_| file(vec![1; small])).collect());
        let start = Instant::now();
        for _ in 0..3 {
            assert_eq!(client.download_media(on_eight(&client, 2, small)).await.unwrap().len(), small);
        }
        assert_eq!((start.elapsed(), drain(&mut rx).len()), (Duration::ZERO, 3));
    }

    #[tokio::test(start_paused = true)]
    async fn a_bots_download_follows_the_rate_set_on_the_builder() {
        let c = CHUNK as usize;
        // Unpaced, and a chunk a second.
        for (rate, turn) in [(None, Duration::ZERO), (Some(CHUNK as u64), Duration::from_secs(1))] {
            let (mut client, mut rx) = Client::scripted((0..16).map(|_| file(vec![1; c])).collect());
            let lane = Lane { connections: None, max_bytes_per_sec: Some(rate) };
            Arc::get_mut(&mut client.0).unwrap().transfer_settings.download = lane;
            let (start, location) = (Instant::now(), on_eight(&client, 2, 16 * c));
            let download = async { client.download_media(location).await };
            let (bytes, after) = tokio::join!(download, sent_after(&mut rx, start, 16));
            assert_eq!(bytes.unwrap().len(), 16 * c);
            assert_eq!(after, (0..16).map(|k| turn * k).collect::<Vec<_>>());
        }
    }
}
