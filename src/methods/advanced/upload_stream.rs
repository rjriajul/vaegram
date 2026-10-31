use tokio::io::AsyncRead;

use super::upload_file::UploadFile;
use crate::Client;

impl Client {
    /// Like [`upload_file`](Client::upload_file), reading exactly `size` bytes from `stream`; `name`'s extension
    /// picks the MIME type.
    ///
    /// A stream shorter than `size` fails with `Error::Io` (`UnexpectedEof`); a longer one keeps the rest. A read
    /// error isn't retried, since the stream can't be rewound. After a failure or a drop, the stream's position is
    /// unspecified. [`stop_transmission`](Client::stop_transmission) doesn't interrupt a read that is waiting on a
    /// stalled stream; drop the future instead.
    pub fn upload_stream<'a>(&'a self, stream: impl AsyncRead + Send + 'a, size: u64, name: &'a str) -> UploadFile<'a> {
        UploadFile { save: self.save_stream(stream, size, name) }
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::functions::upload::SaveFilePart;
    use crate::raw::{Identifiable, Serializable, base};

    const PART: usize = 512 * 1024;

    #[tokio::test]
    async fn uploads_a_two_part_stream() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes()), Ok(true.to_bytes())]);
        // Two workers lease two sessions; both are the scripted one, so nothing goes to the network.
        client.0.transfers.insert(client.dc(2).unwrap(), &client.session());
        let data: Vec<u8> = (0..PART + 1).map(|i| i as u8).collect();
        let file = client.upload_stream(&data[..], data.len() as u64, "a.bin").await.unwrap();
        let base::InputFile::InputFile(f) = &file.0 else { panic!("not a small file") };
        assert_eq!((f.parts, file.name()), (2, "a.bin"));
        let sent: Vec<_> = std::iter::from_fn(|| sent.try_recv().ok()).collect();
        let part = |file_part: i32, bytes: &[u8]| SaveFilePart { file_id: f.id, file_part, bytes: bytes.to_vec() };
        assert_eq!(sent.len(), 2);
        for part in [part(0, &data[..PART]), part(1, &data[PART..])] {
            assert!(sent.contains(&part.to_bytes()), "part {} not sent", part.file_part);
        }
    }

    #[tokio::test]
    async fn a_short_stream_fails_with_an_io_error_before_sending() {
        let (client, mut sent) = Client::recording();
        let e = client.upload_stream(&b"hello"[..], 10, "a.bin").await.err();
        assert!(matches!(&e, Some(Error::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof), "{e:?}");
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_zero_size_stream_is_refused_before_any_request() {
        let (client, mut sent) = Client::recording();
        {
            let mut me = client.0.me.lock().unwrap();
            (me.is_bot, me.premium) = (false, None);
        }
        let e = client.upload_stream(&b"ignored"[..], 0, "a.bin").await.err();
        assert!(matches!(e, Some(Error::FileSize { size: 0, .. })), "{e:?}");
        let save_part = SaveFilePart::CONSTRUCTOR_ID.to_le_bytes();
        assert!(std::iter::from_fn(|| sent.try_recv().ok()).all(|b| b[..4] != save_part));
    }
}
