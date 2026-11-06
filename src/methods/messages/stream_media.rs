use super::MediaStream;
use crate::Client;
use crate::errors::Error;
use crate::types::FileLocation;

impl Client {
    /// Yields the file in order while later chunks download, like `download_media(media).stream()`.
    ///
    /// For a byte range, use [`DownloadMedia::offset`](super::DownloadMedia::offset) and
    /// [`limit`](super::DownloadMedia::limit): `client.download_media(media).offset(o).limit(n).stream()`.
    pub fn stream_media(&self, media: impl TryInto<FileLocation, Error: Into<Error>>) -> MediaStream<'_> {
        self.download_media(media).stream()
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, types};
    use crate::types::FileLocation;

    const CHUNK: usize = 512 * 1024;

    fn file(bytes: Vec<u8>) -> crate::Result<Vec<u8>> {
        let file = types::upload::File { r#type: base::storage::FileType::FilePartial, mtime: 0, bytes };
        Ok(base::upload::File::File(Box::new(file)).to_bytes())
    }

    /// The chunks a stream yields and the requests it sent, for a file of one full chunk and a short one.
    async fn streamed(stream_media: bool) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
        let (client, mut rx) = Client::scripted(vec![file(vec![1; CHUNK]), file(b"hi".into())]);
        client.0.me.lock().unwrap().premium = Some(false);
        let location = types::InputDocumentFileLocation {
            id: 1,
            access_hash: 2,
            file_reference: b"old".to_vec(),
            thumb_size: String::new(),
        };
        let location = base::InputFileLocation::InputDocumentFileLocation(Box::new(location));
        let location = FileLocation { location, dc_id: Some(2), file_size: None };
        let mut stream =
            if stream_media { client.stream_media(location) } else { client.download_media(location).stream() };
        let mut chunks = Vec::new();
        while let Some(chunk) = stream.next().await.unwrap() {
            chunks.push(chunk);
        }
        (chunks, std::iter::from_fn(|| rx.try_recv().ok()).collect())
    }

    #[tokio::test]
    async fn stream_media_sends_what_download_media_stream_sends() {
        let (chunks, sent) = streamed(true).await;
        assert_eq!((chunks.clone(), sent.len()), (vec![vec![1; CHUNK], b"hi".to_vec()], 2));
        assert_eq!(streamed(false).await, (chunks, sent));
    }
}
