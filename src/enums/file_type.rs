use crate::errors::{Error, Result};
use crate::file_id::FileId;

/// What kind of file a file id stores, as [`FileType::of`] reads it.
///
/// A sender given a [`FileSource::file_id`](crate::types::FileSource::file_id) sends the stored file as it is,
/// without checking its kind beyond photo versus other files; this is how to check it first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
// The order is the number a file id stores for its kind: append new kinds, never reorder.
pub enum FileType {
    /// A preview of a sticker, video or other document; it can only be downloaded. A photo's smaller sizes are
    /// [`Photo`](Self::Photo).
    Thumbnail,
    /// A user's or chat's profile photo; it can only be downloaded.
    ChatPhoto,
    /// A photo.
    Photo,
    /// A voice message.
    Voice,
    /// A video.
    Video,
    /// A general file.
    Document,
    /// A file from a secret chat; vaegram never makes ids of this kind.
    Encrypted,
    /// A file kept only for a while, before it is sent; vaegram never makes ids of this kind.
    Temp,
    /// A sticker.
    Sticker,
    /// A music file.
    Audio,
    /// A GIF, or a video without sound.
    Animation,
    /// The thumbnail of a file from a secret chat; vaegram never makes ids of this kind.
    EncryptedThumbnail,
    /// A chat wallpaper, stored as a photo; vaegram never makes ids of this kind.
    Wallpaper,
    /// A round video message.
    VideoNote,
    /// The raw data of a Telegram Passport file; vaegram never makes ids of this kind.
    SecureRaw,
    /// A Telegram Passport file; vaegram never makes ids of this kind.
    Secure,
    /// A chat background; vaegram never makes ids of this kind.
    Background,
    /// A file sent as a plain document even though it has a more specific kind, such as a video; vaegram never
    /// makes ids of this kind.
    DocumentAsFile,
}

impl FileType {
    /// The kind of file `file_id` stores. A malformed id or a web id fails with [`Error::InvalidArgument`], as
    /// [`FileSource::file_id`](crate::types::FileSource::file_id) does.
    ///
    /// Checking a sticker id before sending it, since a sender takes any stored file:
    ///
    /// ```no_run
    /// use vaegram::enums::FileType;
    /// use vaegram::types::FileSource;
    ///
    /// # async fn send(client: &vaegram::Client, file_id: &str) -> vaegram::Result<()> {
    /// if FileType::of(file_id)? != FileType::Sticker {
    ///     return Err(vaegram::Error::InvalidArgument("not a sticker"));
    /// }
    /// client.send_sticker("me", FileSource::file_id(file_id)?).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn of(file_id: &str) -> Result<FileType> {
        Ok(FileId::decode(file_id).map_err(Error::InvalidArgument)?.file_type)
    }
}

#[cfg(test)]
mod tests {
    use super::FileType;
    use crate::errors::Error;
    use crate::file_id::{FileId, ThumbnailSource};

    #[test]
    fn reads_the_kind_of_ids_vaegram_makes() {
        let source = ThumbnailSource::Thumbnail { file_type: FileType::Photo, size: 'm' };
        let photo = FileId { thumbnail_source: Some(source), ..FileId::new(FileType::Photo, 2, 5, 6, &[1]) };
        assert_eq!(FileType::of(&photo.encode()).unwrap(), FileType::Photo);
        for kind in [FileType::Sticker, FileType::Video, FileType::Audio, FileType::Document] {
            assert_eq!(FileType::of(&FileId::new(kind, 2, 7, 8, &[9]).encode()).unwrap(), kind);
        }
    }

    #[test]
    fn refuses_garbage() {
        for id in ["", "not a file id", "AAAA"] {
            assert!(matches!(FileType::of(id), Err(Error::InvalidArgument(_))), "{id:?}");
        }
    }
}
