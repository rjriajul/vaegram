use std::path::{Path, PathBuf};

use super::{input_document, input_photo};
use crate::errors::{Error, Result};
use crate::file_id::{FileId, FileType};
use crate::raw::{base, types};
use crate::types::{
    Animation, Audio, Document, LivePhoto, Photo, Sticker, Upload, UploadedFile, Video, VideoNote, Voice,
};

#[derive(Debug, Clone)]
pub struct FileSource<'a>(pub(crate) Source<'a>);

#[derive(Debug, Clone)]
pub(crate) enum Source<'a> {
    Upload(Upload<'a>),
    Uploaded(&'a UploadedFile),
    /// An `inputMediaPhoto` or `inputMediaDocument` with every flag off.
    Stored(base::InputMedia),
    /// Fetched by Telegram itself.
    Url(String),
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

    /// Telegram downloads the file. Only an empty or non-HTTP(S) URL fails here, as `InvalidArgument`; a URL
    /// Telegram can't fetch fails when it answers, as `Error::Rpc`. A URL carries no attributes (duration, title,
    /// thumbnail), so only photos and plain documents can use one; other senders fail with `InvalidArgument`.
    pub fn url(url: impl Into<String>) -> Result<Self> {
        let url = url.into();
        let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"));
        let valid = rest.is_some_and(|r| !r.is_empty() && !r.starts_with('/')) && !url.contains(char::is_whitespace);
        if !valid {
            return Err(Error::InvalidArgument("a file URL must be an http or https link"));
        }
        Ok(FileSource(Source::Url(url)))
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

/// Sends a file that is already uploaded, so the send's `.progress(..)` is never called for it.
impl<'a> From<&'a UploadedFile> for FileSource<'a> {
    fn from(file: &'a UploadedFile) -> Self {
        FileSource(Source::Uploaded(file))
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

from_documents!(Document, Animation, Audio, Sticker, Video, VideoNote, Voice, LivePhoto);

#[cfg(test)]
mod tests {
    use super::FileSource;
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::file_id::FileId;
    use crate::raw::functions::messages;
    use crate::raw::functions::upload::{SaveBigFilePart, SaveFilePart};
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::types::{InlineMessageId, InputMedia, UploadedFile};

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

    #[tokio::test]
    async fn sends_urls_as_external_media_and_keeps_fetch_errors() {
        let failed = || Err(Error::Rpc(RpcError::new(400, "WEBPAGE_CURL_FAILED")));
        let (client, mut sent) = Client::scripted(vec![failed(), failed()]);
        let url = "https://example.com/cat.jpg";

        let result = client.send_photo(-5, FileSource::url(url).unwrap()).await;
        assert!(matches!(result, Err(Error::Rpc(e)) if e.name == "WEBPAGE_CURL_FAILED"));
        let photo = types::InputMediaPhotoExternal { spoiler: false, url: url.into(), ttl_seconds: None };
        let photo = base::InputMedia::InputMediaPhotoExternal(Box::new(photo));
        assert!(contains(&sent.try_recv().expect("no request sent"), &photo.to_bytes()));

        let result = client.send_document(-5, FileSource::url(url).unwrap()).await;
        assert!(matches!(result, Err(Error::Rpc(e)) if e.name == "WEBPAGE_CURL_FAILED"));
        let document = base::InputMedia::InputMediaDocumentExternal(Box::new(types::InputMediaDocumentExternal {
            spoiler: false,
            url: url.into(),
            ttl_seconds: None,
            video_cover: None,
            video_timestamp: None,
        }));
        assert!(contains(&sent.try_recv().expect("no request sent"), &document.to_bytes()));
    }

    #[tokio::test]
    async fn rejects_bad_urls_and_live_photos_by_url_without_a_request() {
        for url in ["", "example.com/a.jpg", "ftp://example.com/a", "https://", "https:///a", "https://a b"] {
            let e = FileSource::url(url).err();
            assert!(matches!(e, Some(Error::InvalidArgument(_))), "{url}: {e:?}");
        }

        let (client, mut sent) = Client::recording();
        let url = || FileSource::url("https://example.com/a.ogg").unwrap();
        let errors = [
            client.send_live_photo(-5, url(), FileSource::file_id(DOCUMENT).unwrap()).await.err(),
            client.send_live_photo(-5, FileSource::file_id(PHOTO).unwrap(), url()).await.err(),
            client.send_voice(-5, url()).await.err(),
            client.send_video_note(-5, url()).await.err(),
            client.send_sticker(-5, url()).await.err(),
            client.send_animation(-5, url()).await.err(),
            client.send_audio(-5, url()).await.err(),
            client.send_video(-5, url()).await.err(),
            client.send_document(-5, url()).thumb("thumb.jpg").await.err(),
        ];
        for e in errors {
            assert!(matches!(e, Some(Error::InvalidArgument(_))), "{e:?}");
        }
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn sends_uploaded_files_and_keeps_telegram_errors() {
        let missing = || Err(Error::Rpc(RpcError::new(400, "FILE_PART_0_MISSING")));
        let (client, mut sent) = Client::scripted(vec![missing(), missing(), missing()]);
        let input = |name: &str| {
            let file = types::InputFile { id: 7, parts: 1, name: name.into(), md5_checksum: String::new() };
            base::InputFile::InputFile(Box::new(file))
        };
        let (file, other) = (UploadedFile(input("a.jpg")), UploadedFile(input("b.txt")));

        let result = client.send_photo(-5, &file).await;
        assert!(matches!(result, Err(Error::Rpc(e)) if e.name == "FILE_PART_0_MISSING"));
        let photo = base::InputMedia::InputMediaUploadedPhoto(Box::new(types::InputMediaUploadedPhoto {
            spoiler: false,
            live_photo: false,
            file: input("a.jpg"),
            stickers: None,
            ttl_seconds: None,
            video: None,
        }));
        let body = sent.try_recv().expect("no request sent");
        assert_eq!(body[..4], messages::SendMedia::CONSTRUCTOR_ID.to_le_bytes());
        assert!(contains(&body, &photo.to_bytes()));
        assert!(sent.try_recv().is_err());

        let album = [InputMedia::document(&file), InputMedia::document(&other)];
        let result = client.send_media_group(-5, album).await;
        assert!(matches!(result, Err(Error::Rpc(e)) if e.name == "FILE_PART_0_MISSING"));
        let bodies: Vec<_> = std::iter::from_fn(|| sent.try_recv().ok()).collect();
        assert_eq!(bodies[0][..4], messages::UploadMedia::CONSTRUCTOR_ID.to_le_bytes());
        let save_parts = [SaveFilePart::CONSTRUCTOR_ID, SaveBigFilePart::CONSTRUCTOR_ID].map(u32::to_le_bytes);
        assert!(bodies.iter().all(|b| !save_parts.contains(&b[..4].try_into().unwrap())));

        let id = types::InputBotInlineMessageID { dc_id: 2, id: 1, access_hash: 1 };
        let id = InlineMessageId(base::InputBotInlineMessageID::InputBotInlineMessageID(Box::new(id)));
        let e = client.edit_inline_media(id, InputMedia::photo(&file)).await.err();
        assert!(matches!(e, Some(Error::InvalidArgument(_))), "{e:?}");
        assert!(sent.try_recv().is_err());
    }

    fn contains(body: &[u8], part: &[u8]) -> bool {
        body.windows(part.len()).any(|w| w == part)
    }
}
