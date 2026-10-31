use std::pin::Pin;
use std::sync::atomic::AtomicI32;

use tokio::io::{AsyncRead, AsyncReadExt};

use crate::Client;
use crate::crypto::random;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::session::Session;
use crate::session::transfer::{Progress, Report, invoke_part, take, try_join_all, workers};
use crate::types::Upload;

// https://core.telegram.org/api/files#uploading-files
const PART_SIZE: u64 = 512 * 1024;
const BIG_FILE_SIZE: u64 = 10 * 1024 * 1024;
const MAX_PARTS: u64 = 4000;
const MAX_PARTS_PREMIUM: u64 = 8000;
const IN_FLIGHT: u64 = 4 << 20;
const CONNECTIONS: usize = 4;
// The server throttles Premium uploads less, so more connections stay busy.
const CONNECTIONS_PREMIUM: usize = 8;

pub(crate) struct SaveFile<'a> {
    client: &'a Client,
    input: Input<'a>,
    progress: Option<Report<'a>>,
}

enum Input<'a> {
    Upload(Upload<'a>),
    Stream { reader: Pin<Box<dyn AsyncRead + Send + 'a>>, size: u64, name: &'a str },
}

impl Client {
    pub(crate) fn save_file<'a>(&'a self, file: impl Into<Upload<'a>>) -> SaveFile<'a> {
        SaveFile { client: self, input: Input::Upload(file.into()), progress: None }
    }

    pub(crate) fn save_stream<'a>(
        &'a self,
        stream: impl AsyncRead + Send + 'a,
        size: u64,
        name: &'a str,
    ) -> SaveFile<'a> {
        let input = Input::Stream { reader: Box::pin(stream), size, name };
        SaveFile { client: self, input, progress: None }
    }
}

impl<'a> SaveFile<'a> {
    pub(crate) fn progress(mut self, progress: Option<Report<'a>>) -> Self {
        self.progress = progress;
        self
    }

    async fn send(self) -> Result<base::InputFile> {
        let (source, size, name) = match self.input {
            Input::Upload(upload @ Upload::Path(path)) => {
                let disk = tokio::fs::File::open(path).await?;
                let size = disk.metadata().await?.len();
                (Source::File(tokio::sync::Mutex::new(disk)), size, upload.name())
            }
            Input::Upload(upload @ Upload::Bytes { data, .. }) => {
                (Source::Bytes(data), data.len() as u64, upload.name())
            }
            Input::Stream { reader, size, name } => (Source::Stream(tokio::sync::Mutex::new(reader)), size, name),
        };
        // Checked before the limits: looking up Premium can cost a request.
        if size == 0 {
            return Err(Error::FileSize { size: 0, limit: MAX_PARTS * PART_SIZE });
        }
        let (parts, workers, connections) = self.client.upload_limits(size).await?;
        let home = self.client.dc(self.client.session().dc_id())?;
        let lease = self.client.transfer_sessions(home, connections).await?;
        let upload = Parts { source, size, count: parts, big: size > BIG_FILE_SIZE, id: i64::from_le_bytes(random()) };
        let (next, progress) = (AtomicI32::new(0), Progress::new(self.progress, size));
        let sessions = &lease.sessions;
        try_join_all((0..workers).map(|i| upload.send(&sessions[i % sessions.len()], &next, &progress))).await?;
        Ok(upload.input_file(name))
    }
}

into_future!(SaveFile, base::InputFile);

impl Client {
    async fn upload_limits(&self, size: u64) -> Result<(i32, usize, usize)> {
        let premium = match self.is_premium().await {
            Err(e) if size > MAX_PARTS * PART_SIZE => return Err(e),
            premium => premium.unwrap_or(false),
        };
        let parts = part_count(size, premium)?;
        let workers = workers(parts, PART_SIZE, IN_FLIGHT);
        Ok((parts, workers, workers.min(if premium { CONNECTIONS_PREMIUM } else { CONNECTIONS })))
    }
}

fn part_count(size: u64, premium: bool) -> Result<i32> {
    let limit = if premium { MAX_PARTS_PREMIUM } else { MAX_PARTS } * PART_SIZE;
    if size == 0 || size > limit {
        return Err(Error::FileSize { size, limit });
    }
    Ok(size.div_ceil(PART_SIZE) as i32)
}

enum Source<'a> {
    File(tokio::sync::Mutex<tokio::fs::File>),
    Stream(tokio::sync::Mutex<Pin<Box<dyn AsyncRead + Send + 'a>>>),
    Bytes(&'a [u8]),
}

struct Parts<'a> {
    source: Source<'a>,
    size: u64,
    count: i32,
    big: bool,
    id: i64,
}

impl Parts<'_> {
    fn len(&self, part: i32) -> usize {
        (self.size - part as u64 * PART_SIZE).min(PART_SIZE) as usize
    }

    async fn read(&self, next: &AtomicI32, buf: &mut Vec<u8>) -> Result<Option<i32>> {
        match &self.source {
            Source::File(file) => self.read_locked(file, next, buf).await,
            Source::Stream(stream) => self.read_locked(stream, next, buf).await,
            Source::Bytes(data) => {
                let Some(part) = take(next, self.count) else { return Ok(None) };
                let start = part as usize * PART_SIZE as usize;
                buf.clear();
                buf.extend_from_slice(&data[start..start + self.len(part)]);
                Ok(Some(part))
            }
        }
    }

    /// The part number is taken under the lock, so parts are read in order.
    async fn read_locked<R: AsyncRead + Unpin + Send>(
        &self,
        reader: &tokio::sync::Mutex<R>,
        next: &AtomicI32,
        buf: &mut Vec<u8>,
    ) -> Result<Option<i32>> {
        let mut reader = reader.lock().await;
        let Some(part) = take(next, self.count) else { return Ok(None) };
        buf.resize(self.len(part), 0);
        reader.read_exact(buf).await?;
        Ok(Some(part))
    }

    async fn send(&self, session: &Session, next: &AtomicI32, progress: &Progress<'_>) -> Result<()> {
        let (file_id, mut bytes) = (self.id, Vec::with_capacity(PART_SIZE.min(self.size) as usize));
        while let Some(file_part) = self.read(next, &mut bytes).await? {
            let saved = if self.big {
                let total = self.count;
                let request = functions::upload::SaveBigFilePart { file_id, file_part, file_total_parts: total, bytes };
                let saved = invoke_part(session, &request).await;
                bytes = request.bytes;
                saved
            } else {
                let request = functions::upload::SaveFilePart { file_id, file_part, bytes };
                let saved = invoke_part(session, &request).await;
                bytes = request.bytes;
                saved
            };
            if !saved? {
                return Err(Error::Unexpected("upload.saveFilePart returned false"));
            }
            progress.add(bytes.len());
        }
        Ok(())
    }

    fn input_file(&self, name: &str) -> base::InputFile {
        let (id, parts, name) = (self.id, self.count, name.to_owned());
        if self.big {
            base::InputFile::InputFileBig(Box::new(types::InputFileBig { id, parts, name }))
        } else {
            base::InputFile::InputFile(Box::new(types::InputFile { id, parts, name, md5_checksum: String::new() }))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicI32;

    use super::{BIG_FILE_SIZE, MAX_PARTS, MAX_PARTS_PREMIUM, PART_SIZE, Parts, Source, part_count};
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::functions::users::GetUsers;
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::types::Upload;

    fn premium_user() -> Vec<u8> {
        let mut user = Vec::new();
        for word in [1u32 << 28, 0] {
            word.serialize(&mut user);
        }
        7i64.serialize(&mut user);
        let user = types::User::deserialize_bare(&mut &user[..]).unwrap();
        let mut reply = Vec::new();
        vec![base::User::User(Box::new(user))].serialize(&mut reply);
        reply
    }

    #[tokio::test]
    async fn uploads_with_regular_limits_when_the_account_lookup_fails() {
        let failed = Err(Error::Rpc(RpcError::new(500, "INTERNAL")));
        let (client, mut sent) = Client::scripted(vec![failed, Ok(premium_user())]);
        client.0.me.lock().unwrap().is_bot = false;
        let get_users = GetUsers::CONSTRUCTOR_ID.to_le_bytes();
        let mut lookups = || std::iter::from_fn(|| sent.try_recv().ok()).filter(|b| b[..4] == get_users).count();
        let big = MAX_PARTS * PART_SIZE + 1;

        assert_eq!(client.upload_limits(3 << 20).await.unwrap(), (6, 6, 4));
        assert!(matches!(client.upload_limits(big).await, Ok((4001, 8, 8))));
        assert_eq!(lookups(), 2);
        assert_eq!(client.upload_limits(3 << 20).await.unwrap(), (6, 6, 6));
        assert_eq!(lookups(), 0);

        client.0.events.reconnected();
        assert!(matches!(client.upload_limits(big).await, Err(Error::Disconnected)));
        assert_eq!(lookups(), 1);
    }

    #[test]
    fn counts_512k_parts_up_to_the_account_limit() {
        assert!(matches!(part_count(0, false), Err(Error::FileSize { size: 0, .. })));
        let count = |size| part_count(size, false).unwrap();
        assert_eq!(
            (count(1), count(PART_SIZE), count(PART_SIZE + 1), count(BIG_FILE_SIZE), count(BIG_FILE_SIZE + 1)),
            (1, 1, 2, 20, 21)
        );
        assert_eq!(count(MAX_PARTS * PART_SIZE), 4000);
        assert!(matches!(part_count(MAX_PARTS * PART_SIZE + 1, false), Err(Error::FileSize { .. })));
        assert_eq!(part_count(MAX_PARTS * PART_SIZE + 1, true).unwrap(), 4001);
        assert!(matches!(part_count(MAX_PARTS_PREMIUM * PART_SIZE + 1, true), Err(Error::FileSize { .. })));
    }

    fn stream(data: &[u8]) -> Source<'_> {
        Source::Stream(tokio::sync::Mutex::new(Box::pin(data)))
    }

    async fn split(data: &[u8]) -> Vec<usize> {
        split_from(Source::Bytes(data), data).await
    }

    async fn split_from(source: Source<'_>, data: &[u8]) -> Vec<usize> {
        let count = part_count(data.len() as u64, false).unwrap();
        let big = data.len() as u64 > BIG_FILE_SIZE;
        let parts = Parts { source, size: data.len() as u64, count, big, id: 7 };
        let (next, mut sizes, mut joined, mut buf) = (AtomicI32::new(0), Vec::new(), Vec::new(), Vec::new());
        while let Some(n) = parts.read(&next, &mut buf).await.unwrap() {
            assert_eq!(n as usize, sizes.len());
            sizes.push(buf.len());
            joined.extend_from_slice(&buf);
        }
        assert_eq!(joined, data);
        sizes
    }

    #[tokio::test]
    async fn reads_parts_in_order() {
        let part = PART_SIZE as usize;
        assert_eq!(split(b"a").await, [1]);
        assert_eq!(split(&vec![1; part]).await, [part]);
        assert_eq!(split(&vec![1; part + 1]).await, [part, 1]);
        let big = split(&vec![2; BIG_FILE_SIZE as usize + 1]).await;
        assert_eq!((big.len(), big[20]), (21, 1));
    }

    #[tokio::test]
    async fn reads_a_stream_in_part_order() {
        let part = PART_SIZE as usize;
        let data: Vec<u8> = (0..part + 1).map(|i| i as u8).collect();
        assert_eq!(split_from(stream(&data[..1]), &data[..1]).await, [1]);
        assert_eq!(split_from(stream(&data[..part]), &data[..part]).await, [part]);
        assert_eq!(split_from(stream(&data), &data).await, [part, 1]);
    }

    #[tokio::test]
    async fn a_short_stream_fails_with_unexpected_eof() {
        let parts = Parts { source: stream(b"hello"), size: 10, count: 1, big: false, id: 7 };
        let e = parts.read(&AtomicI32::new(0), &mut Vec::new()).await.err();
        assert!(matches!(&e, Some(Error::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof), "{e:?}");
    }

    #[tokio::test]
    async fn an_empty_upload_is_refused_before_any_request() {
        let (client, mut sent) = Client::recording();
        {
            let mut me = client.0.me.lock().unwrap();
            (me.is_bot, me.premium) = (false, None);
        }
        let empty = [client.save_file(Upload::bytes("a.txt", b"")).await, client.save_stream(&b""[..], 0, "a").await];
        for e in empty {
            assert!(matches!(e, Err(Error::FileSize { size: 0, .. })), "{e:?}");
        }
        assert!(sent.try_recv().is_err());
    }

    #[test]
    fn switches_to_big_files_above_10_mb() {
        let file = |size: u64| {
            let big = size > BIG_FILE_SIZE;
            let count = part_count(size, false).unwrap();
            Parts { source: Source::Bytes(&[]), size, count, big, id: 7 }.input_file("a.bin")
        };
        let base::InputFile::InputFile(small) = file(BIG_FILE_SIZE) else { panic!("10 MB is still a small file") };
        assert_eq!((small.id, small.parts, small.name.as_str(), small.md5_checksum.as_str()), (7, 20, "a.bin", ""));
        assert!(matches!(file(BIG_FILE_SIZE + 1), base::InputFile::InputFileBig(f) if f.parts == 21));
    }
}
