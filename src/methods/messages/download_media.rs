use std::future::poll_fn;
use std::io::SeekFrom;
use std::path::Path;
use std::pin::Pin;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::task::Poll;

use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{mpsc, watch};

use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::session::Session;
use crate::session::cdn::Cdn;
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
    source: Option<(i64, i32)>,
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
        DownloadMedia::new(self, media.try_into().map_err(Into::into), None)
    }
}

impl<'a> DownloadMedia<'a> {
    /// `source` is the chat and message the file came from, re-read for a fresh file_reference once it expires.
    pub(crate) fn new(client: &'a Client, location: Result<FileLocation>, source: Option<(i64, i32)>) -> Self {
        DownloadMedia { client, location, source, progress: None }
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
        let client = self.client;
        let location = self.location?;
        let size = location.file_size;
        let count = chunk_count(size);
        let progress = Progress::new(self.progress, size.and_then(|s| u64::try_from(s).ok()).unwrap_or(0));
        let budget = if client.is_premium().await.unwrap_or(false) { IN_FLIGHT_PREMIUM } else { IN_FLIGHT };
        // Without a size the end is only known from a short chunk, so chunks go one at a time.
        let workers = count.map_or(1, |c| workers(c, CHUNK as u64, budget));
        let mut request = functions::upload::GetFile {
            precise: false,
            cdn_supported: true,
            location: location.location,
            offset: 0,
            limit: CHUNK,
        };

        let mut dc_id = location.dc_id.unwrap_or_else(|| client.session().dc_id());
        let (mut migrations, mut refreshed) = (0, false);
        let (lease, file) = loop {
            let lease = client.transfer_sessions(client.media_dc(dc_id)?, workers.min(MAX_CONNECTIONS)).await?;
            match invoke_part(&lease.sessions[0], &request).await {
                Ok(file) => break (lease, file),
                Err(Error::Rpc(e)) if e.name == "FILE_MIGRATE" && migrations < MAX_MIGRATIONS => {
                    dc_id = e.value.ok_or(Error::Unexpected("FILE_MIGRATE without a DC number"))? as i32;
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
            client,
            sessions: &lease.sessions,
            source: self.source,
            location: Mutex::new((request.location, 0)),
            renewing: tokio::sync::Mutex::new(()),
        };
        let master = &master;
        match file {
            base::upload::File::File(f) => {
                write_all(f.bytes, size, workers, sink, &progress, move |i, offset| master.part(i, offset)).await
            }
            base::upload::File::FileCdnRedirect(r) => from_cdn(master, *r, size, workers, sink, &progress).await,
        }
    }
}

/// The DC that stores the file. Chunks share its file_reference, renewed once when several expire together.
struct Master<'a> {
    client: &'a Client,
    sessions: &'a [Session],
    source: Option<(i64, i32)>,
    location: Mutex<(base::InputFileLocation, usize)>,
    renewing: tokio::sync::Mutex<()>,
}

impl Master<'_> {
    async fn part(&self, worker: usize, offset: i64) -> Result<Vec<u8>> {
        let session = &self.sessions[worker % self.sessions.len()];
        let mut renewed = false;
        loop {
            let (location, seen) = self.location.lock().unwrap().clone();
            let request =
                functions::upload::GetFile { precise: false, cdn_supported: false, location, offset, limit: CHUNK };
            match invoke_part(session, &request).await {
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

// The master DC is trusted, so once the CDN fails in any way (FILE_TOKEN_INVALID, a lost key, a bad hash) it serves
// the chunk that failed and every later one; chunks already written stay as they are.
async fn from_cdn(
    master: &Master<'_>,
    redirect: types::upload::FileCdnRedirect,
    size: Option<i64>,
    workers: usize,
    sink: &Sink,
    progress: &Progress<'_>,
) -> Result<()> {
    let client = master.client;
    let cdn = async {
        let cdn = Cdn::new(&master.sessions[0], redirect, (client.0.api_id, &client.0.options))?;
        let dc = client.dc(cdn.dc_id)?;
        let lease = client.cdn_sessions(dc, workers.min(MAX_CONNECTIONS)).await?;
        Ok::<_, Error>((cdn, dc, lease))
    }
    .await
    .ok();
    let (cdn, failed) = (&cdn, &AtomicBool::new(cdn.is_none()));
    let part = move |i: usize, offset: i64| async move {
        if let (Some((cdn, dc, lease)), false) = (cdn, failed.load(Ordering::Acquire)) {
            match cdn.part(&lease.sessions[i % lease.sessions.len()], offset, CHUNK).await {
                Ok(bytes) => return Ok(bytes),
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
    write_all(part(0, 0).await?, size, workers, sink, progress, part).await
}

async fn write_all<F: Future<Output = Result<Vec<u8>>>>(
    first: Vec<u8>,
    size: Option<i64>,
    workers: usize,
    sink: &Sink,
    progress: &Progress<'_>,
    part: impl Fn(usize, i64) -> F,
) -> Result<()> {
    fits(0, &first, size)?;
    let len = first.len();
    sink.write_at(0, first).await?;
    progress.add(len);
    if len < CHUNK as usize {
        return Ok(());
    }
    let (next, part) = (AtomicI32::new(1), &part);
    try_join_all((0..workers).map(|i| chunks(&next, size, sink, progress, move |offset| part(i, offset)))).await?;
    Ok(())
}

async fn chunks<F: Future<Output = Result<Vec<u8>>>>(
    next: &AtomicI32,
    size: Option<i64>,
    sink: &Sink,
    progress: &Progress<'_>,
    part: impl Fn(i64) -> F,
) -> Result<()> {
    while let Some(n) = take(next, chunk_count(size).unwrap_or(i32::MAX)) {
        let (offset, _) = chunk(n);
        let bytes = part(offset).await?;
        fits(offset, &bytes, size)?;
        let len = bytes.len();
        sink.write_at(offset as u64, bytes).await?;
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

/// False when there is no message to re-read or it no longer holds this file.
async fn refresh(client: &Client, source: Option<(i64, i32)>, location: &mut base::InputFileLocation) -> Result<bool> {
    let Some((chat_id, message_id)) = source else { return Ok(false) };
    let (file_id, reference) = match location {
        base::InputFileLocation::InputPhotoFileLocation(l) => (l.id, &mut l.file_reference),
        base::InputFileLocation::InputDocumentFileLocation(l) => (l.id, &mut l.file_reference),
        _ => return Ok(false),
    };
    let (found, _) = client.get_raw_messages(chat_id, &[message_id]).await?;
    let Some(fresh) = found.into_iter().flatten().find_map(|m| fresh_reference(m.media?, file_id)) else {
        return Ok(false);
    };
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
    use std::sync::atomic::AtomicI32;
    use std::sync::{Arc, Mutex};

    use tokio::sync::{mpsc, watch};

    use super::{CHUNK, DownloadMedia, Master, MediaStream, Sink, chunk, chunk_count, fits};
    use crate::Client;
    use crate::crypto::{aes, sha256};
    use crate::errors::{Error, Result, RpcError};
    use crate::raw::{Identifiable, Serializable, base, functions, types};
    use crate::session::Session;
    use crate::session::transfer::{take, try_join_all};
    use crate::types::messages_and_media::media::tests::document_media;
    use crate::types::{FileLocation, Message};
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
        source: Option<(i64, i32)>,
    ) -> (Result<Vec<u8>, String>, Vec<u64>) {
        client.0.me.lock().unwrap().premium = Some(false);
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

    fn drain(rx: &mut mpsc::UnboundedReceiver<Vec<u8>>) -> Vec<Vec<u8>> {
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    #[tokio::test]
    async fn refetches_the_message_once_for_an_expired_reference() {
        let get_messages = functions::messages::GetMessages::CONSTRUCTOR_ID.to_le_bytes();
        let (old, fresh) = (get_file(b"old", 0, true).to_bytes(), get_file(&[3], 0, true).to_bytes());
        let expired = || Err("FILE_REFERENCE_EXPIRED".into());
        let source = Some((-5, 5));

        let (client, mut rx) = Client::scripted(vec![error("FILE_REFERENCE_EXPIRED"), messages(), file(b"hi".into())]);
        let (result, _) = download(&client, Some(5), source).await;
        let sent = drain(&mut rx);
        assert_eq!((result, [&sent[0], &sent[2]]), (Ok(b"hi".to_vec()), [&old, &fresh]));
        assert_eq!(sent[1][..4], get_messages);

        let replies = vec![error("FILE_REFERENCE_EXPIRED"), messages(), error("FILE_REFERENCE_EXPIRED")];
        let (client, mut rx) = Client::scripted(replies);
        assert_eq!((download(&client, Some(5), source).await.0, drain(&mut rx).len()), (expired(), 3));
        let (client, mut rx) = Client::scripted(vec![error("FILE_REFERENCE_EXPIRED")]);
        assert_eq!((download(&client, Some(5), None).await.0, drain(&mut rx)), (expired(), vec![old]));
    }

    #[tokio::test]
    async fn renews_a_reference_that_expires_mid_file() {
        let c = CHUNK as usize;
        let replies = vec![file(vec![1; c]), error("FILE_REFERENCE_EXPIRED"), messages(), file(b"hi".into())];
        let (client, mut rx) = Client::scripted(replies);
        let (result, done) = download(&client, None, Some((-5, 5))).await;
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
            client: &client,
            sessions: &sessions,
            source: Some((-5, 5)),
            location: Mutex::new((get_file(b"old", 0, false).location, 0)),
            renewing: tokio::sync::Mutex::new(()),
        };
        let parts = try_join_all([master.part(0, 0), master.part(1, i64::from(CHUNK))].into_iter()).await.unwrap();
        let get_messages = functions::messages::GetMessages::CONSTRUCTOR_ID.to_le_bytes();
        let sent = drain(&mut rx);
        let renewals = sent.iter().filter(|b| b[..4] == get_messages).count();
        assert_eq!((parts, renewals, sent.len()), (vec![b"a".to_vec(), b"b".to_vec()], 1, 5));
        assert_eq!(master.location.lock().unwrap().1, 1);
    }

    #[tokio::test]
    async fn the_master_dc_serves_the_rest_once_the_cdn_fails() {
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
        let (cdn, mut cdn_rx) = Session::scripted(vec![Ok(part.to_bytes()), error("FILE_TOKEN_INVALID")]);

        let (client, mut rx) = Client::scripted(vec![redirect, file(b"hi".into())]);
        client.serve_cdn(203, &cdn);
        let (result, done) = download(&client, None, None).await;
        assert_eq!((result, done), (Ok([plain, b"hi".to_vec()].concat()), vec![c as u64, c as u64 + 2]));
        let master = [get_file(b"old", 0, true).to_bytes(), get_file(b"old", i64::from(CHUNK), false).to_bytes()];
        assert_eq!((drain(&mut rx), drain(&mut cdn_rx).len()), (master.to_vec(), 2));
    }
}
