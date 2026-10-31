use super::save_file::SaveFile;
use crate::Client;
use crate::errors::Result;
use crate::types::{Upload, UploadedFile};

/// Awaiting it uploads the file and gives an [`UploadedFile`] to pass to any media sender as `&file`.
#[must_use = "a request does nothing until it is awaited"]
pub struct UploadFile<'a> {
    pub(super) save: SaveFile<'a>,
}

impl Client {
    /// Uploads a file from a path or [`Upload::bytes`] without sending it, so it can be sent later or more than once.
    ///
    /// Telegram keeps the parts "between several minutes and several hours"
    /// (<https://core.telegram.org/api/files#uploading-files>); after that a send fails with `Error::Rpc`
    /// (`FILE_PART_…_MISSING`). Send it soon; it can be sent several times within that window. For longer reuse,
    /// resend the sent message's media. The name's extension picks the MIME type. An empty file fails with
    /// `Error::FileSize` before any request. [`stop_transmission`](Client::stop_transmission) ends every transfer;
    /// drop the future to cancel only this one.
    pub fn upload_file<'a>(&'a self, file: impl Into<Upload<'a>>) -> UploadFile<'a> {
        UploadFile { save: self.save_file(file) }
    }
}

impl<'a> UploadFile<'a> {
    /// Calls `progress(done, total)` with byte counts after each part.
    pub fn progress(mut self, progress: impl FnMut(u64, u64) + Send + 'a) -> Self {
        self.save = self.save.progress(Some(Box::new(progress)));
        self
    }

    async fn send(self) -> Result<UploadedFile> {
        Ok(UploadedFile(self.save.await?))
    }
}

into_future!(UploadFile, UploadedFile);

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use crate::Client;
    use crate::raw::functions::upload::SaveFilePart;
    use crate::raw::{Serializable, base};
    use crate::types::Upload;

    #[tokio::test]
    async fn uploads_bytes_and_keeps_the_name() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes())]);
        let file = client.upload_file(Upload::bytes("a.txt", b"hi")).await.unwrap();
        assert_eq!(file.name(), "a.txt");
        let base::InputFile::InputFile(f) = &file.0 else { panic!("not a small file") };
        assert_eq!(f.parts, 1);
        let part = SaveFilePart { file_id: f.id, file_part: 0, bytes: b"hi".to_vec() };
        assert_eq!(sent.try_recv().unwrap(), part.to_bytes());
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn progress_counts_every_byte() {
        let (client, _sent) = Client::scripted(vec![Ok(true.to_bytes())]);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let upload = client.upload_file(Upload::bytes("a.txt", b"hi"));
        upload.progress(move |done, total| log.lock().unwrap().push((done, total))).await.unwrap();
        assert_eq!(*seen.lock().unwrap(), [(2, 2)]);
    }
}
