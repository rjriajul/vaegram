use std::path::{Path, PathBuf};

use super::{input_document, input_photo};
use crate::errors::{Error, Result};
use crate::file_id::{FileId, FileType};
use crate::raw::{base, types};
use crate::types::{Animation, Audio, Document, Photo, Sticker, Upload, Video, VideoNote, Voice};

#[derive(Debug, Clone)]
pub struct FileSource<'a>(pub(crate) Source<'a>);

#[derive(Debug, Clone)]
pub(crate) enum Source<'a> {
    Upload(Upload<'a>),
    /// An `inputMediaPhoto` or `inputMediaDocument` with every flag off.
    Stored(base::InputMedia),
}

impl FileSource<'_> {
    /// Only a malformed id, a web id or one that can only be downloaded fails here, as `InvalidArgument`; a stale id
    /// or one from another bot fails when Telegram answers, as `Error::Rpc`.
    pub fn file_id(file_id: &str) -> Result<Self> {
        let id = FileId::decode(file_id).map_err(Error::InvalidArgument)?;
        if matches!(id.file_type, FileType::Thumbnail | FileType::ChatPhoto) {
            return Err(Error::InvalidArgument("this file id can only be downloaded"));
        }
        let (photo, file_reference) = (id.file_type.is_photo(), id.file_reference);
        let (id, access_hash) = (id.media_id, id.access_hash);
        Ok(if photo {
            stored_photo(base::InputPhoto::InputPhoto(Box::new(types::InputPhoto { id, access_hash, file_reference })))
        } else {
            let document = types::InputDocument { id, access_hash, file_reference };
            stored_document(base::InputDocument::InputDocument(Box::new(document)))
        })
    }
}

fn stored_photo<'a>(id: base::InputPhoto) -> FileSource<'a> {
    let media = types::InputMediaPhoto { spoiler: false, live_photo: false, id, ttl_seconds: None, video: None };
    FileSource(Source::Stored(base::InputMedia::InputMediaPhoto(Box::new(media))))
}

fn stored_document<'a>(id: base::InputDocument) -> FileSource<'a> {
    let media = types::InputMediaDocument {
        spoiler: false,
        id,
        video_cover: None,
        video_timestamp: None,
        ttl_seconds: None,
        query: None,
    };
    FileSource(Source::Stored(base::InputMedia::InputMediaDocument(Box::new(media))))
}

impl<'a> From<Upload<'a>> for FileSource<'a> {
    fn from(upload: Upload<'a>) -> Self {
        FileSource(Source::Upload(upload))
    }
}

impl<'a> From<&'a str> for FileSource<'a> {
    fn from(path: &'a str) -> Self {
        FileSource(Source::Upload(path.into()))
    }
}

impl<'a> From<&'a Path> for FileSource<'a> {
    fn from(path: &'a Path) -> Self {
        FileSource(Source::Upload(path.into()))
    }
}

impl<'a> From<&'a PathBuf> for FileSource<'a> {
    fn from(path: &'a PathBuf) -> Self {
        FileSource(Source::Upload(path.into()))
    }
}

impl From<&Photo> for FileSource<'_> {
    fn from(photo: &Photo) -> Self {
        stored_photo(input_photo(&photo.location))
    }
}

macro_rules! from_documents {
    ($($media:ident),*) => {$(
        impl From<&$media> for FileSource<'_> {
            fn from(media: &$media) -> Self {
                stored_document(input_document(&media.location))
            }
        }
    )*};
}

from_documents!(Document, Animation, Audio, Sticker, Video, VideoNote, Voice);

#[cfg(test)]
mod tests {
    use super::FileSource;
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::file_id::FileId;
    use crate::raw::functions::messages;
    use crate::raw::{Identifiable, Serializable, base, types};

    const PHOTO: &str = "AgACAgIAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAIG9DEbXX3wSq3oI7t_PqQGAQADAgADbQADNgQ";
    const DOCUMENT: &str = "BQACAgQAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAL6C87kooIPtf__________HgQ";

    #[tokio::test]
    async fn sends_stored_ids_and_keeps_telegram_errors() {
        let expired = || Err(Error::Rpc(RpcError::new(400, "FILE_REFERENCE_EXPIRED")));
        let (client, mut sent) = Client::scripted(vec![expired(), expired()]);
        let file_reference = FileId::decode(PHOTO).unwrap().file_reference;

        let result = client.send_photo(-5, FileSource::file_id(PHOTO).unwrap()).caption("again").await;
        assert!(matches!(result, Err(Error::Rpc(e)) if e.name == "FILE_REFERENCE_EXPIRED"));
        let body = sent.try_recv().expect("no request sent");
        let photo = types::InputPhoto {
            id: 5399953792058913798,
            access_hash: 478576178729576621,
            file_reference: file_reference.clone(),
        };
        let media = base::InputMedia::InputMediaPhoto(Box::new(types::InputMediaPhoto {
            spoiler: false,
            live_photo: false,
            id: base::InputPhoto::InputPhoto(Box::new(photo)),
            ttl_seconds: None,
            video: None,
        }));
        assert_eq!(body[..4], messages::SendMedia::CONSTRUCTOR_ID.to_le_bytes());
        assert!(contains(&body, &media.to_bytes()));

        let result = client.send_document(-5, FileSource::file_id(DOCUMENT).unwrap()).await;
        assert!(matches!(result, Err(Error::Rpc(e)) if e.name == "FILE_REFERENCE_EXPIRED"));
        let body = sent.try_recv().expect("no request sent");
        let document = types::InputDocument { id: -5399953792058913798, access_hash: -1, file_reference };
        let media = base::InputMedia::InputMediaDocument(Box::new(types::InputMediaDocument {
            spoiler: false,
            id: base::InputDocument::InputDocument(Box::new(document)),
            video_cover: None,
            video_timestamp: None,
            ttl_seconds: None,
            query: None,
        }));
        assert!(contains(&body, &media.to_bytes()));
    }

    #[tokio::test]
    async fn rejects_bad_ids_and_wrong_kinds_without_a_request() {
        let chat_photo = "AQADAQADFc1bBwAUAwADLu3E4Rb___9PlyHF_____wAEHgQ";
        for file_id in [chat_photo, "AQADAQ*bad", "", "BQADAQADKgAHBw"] {
            let e = FileSource::file_id(file_id).err();
            assert!(matches!(e, Some(Error::InvalidArgument(_))), "{file_id}: {e:?}");
        }

        let (client, mut sent) = Client::recording();
        let e = client.send_photo(-5, FileSource::file_id(DOCUMENT).unwrap()).await.err();
        assert!(matches!(e, Some(Error::InvalidArgument(_))), "{e:?}");
        let e = client.send_document(-5, FileSource::file_id(PHOTO).unwrap()).await.err();
        assert!(matches!(e, Some(Error::InvalidArgument(_))), "{e:?}");
        assert!(sent.try_recv().is_err());
    }

    fn contains(body: &[u8], part: &[u8]) -> bool {
        body.windows(part.len()).any(|w| w == part)
    }
}
