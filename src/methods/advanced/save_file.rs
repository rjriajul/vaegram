use std::sync::atomic::AtomicI32;

use tokio::io::AsyncReadExt;

use crate::Client;
use crate::crypto::random;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::session::Session;
use crate::session::transfer::{invoke_part, take, try_join_all, workers};
use crate::types::Upload;

// https://core.telegram.org/api/files#uploading-files
const PART_SIZE: u64 = 512 * 1024;
const BIG_FILE_SIZE: u64 = 10 * 1024 * 1024;
const MAX_PARTS: u64 = 4000;
const MAX_PARTS_PREMIUM: u64 = 8000;
const IN_FLIGHT: u64 = 4 << 20;
const CONNECTIONS: usize = 4;

impl Client {
    pub(crate) async fn save_file(&self, file: impl Into<Upload<'_>>) -> Result<base::InputFile> {
        let file = file.into();
        let (source, size) = match file {
            Upload::Path(path) => {
                let disk = tokio::fs::File::open(path).await?;
                let size = disk.metadata().await?.len();
                (Source::File(tokio::sync::Mutex::new(disk)), size)
            }
            Upload::Bytes { data, .. } => (Source::Bytes(data), data.len() as u64),
        };
        let mut premium = false;
        if size > MAX_PARTS * PART_SIZE {
            premium = matches!(self.get_me().await?, base::User::User(u) if u.premium);
        }
        let parts = part_count(size, premium)?;
        let workers = workers(parts, PART_SIZE, IN_FLIGHT);
        let lease = self.transfer_sessions(self.session().dc_id(), workers.min(CONNECTIONS)).await?;
        let upload = Parts { source, size, count: parts, big: size > BIG_FILE_SIZE, id: i64::from_le_bytes(random()) };
        let next = AtomicI32::new(0);
        try_join_all((0..workers).map(|i| upload.send(&lease.sessions[i % lease.sessions.len()], &next))).await?;
        Ok(upload.input_file(file.name()))
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
    async fn read(&self, next: &AtomicI32, buf: &mut Vec<u8>) -> Result<Option<i32>> {
        let len = |part: i32| (self.size - part as u64 * PART_SIZE).min(PART_SIZE) as usize;
        match &self.source {
            Source::File(file) => {
                let mut file = file.lock().await;
                let Some(part) = take(next, self.count) else { return Ok(None) };
                buf.resize(len(part), 0);
                file.read_exact(buf).await?;
                Ok(Some(part))
            }
            Source::Bytes(data) => {
                let Some(part) = take(next, self.count) else { return Ok(None) };
                let start = part as usize * PART_SIZE as usize;
                buf.clear();
                buf.extend_from_slice(&data[start..start + len(part)]);
                Ok(Some(part))
            }
        }
    }

    async fn send(&self, session: &Session, next: &AtomicI32) -> Result<()> {
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
    use crate::errors::Error;
    use crate::raw::base;

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

    async fn split(data: &[u8]) -> Vec<usize> {
        let count = part_count(data.len() as u64, false).unwrap();
        let big = data.len() as u64 > BIG_FILE_SIZE;
        let parts = Parts { source: Source::Bytes(data), size: data.len() as u64, count, big, id: 7 };
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
