//! Bot API `file_id` and `file_unique_id` strings: little-endian TL fields, zero bytes run-length encoded, then
//! base64url without padding. `file_unique_id` is not yet checked against the ids the Bot API itself gives out.

use crate::raw::{Deserializable, Serializable};
use crate::utils::{base64url_decode as b64_decode, base64url_encode as b64_encode};

const WEB_LOCATION_FLAG: i32 = 1 << 24;
const FILE_REFERENCE_FLAG: i32 = 1 << 25;
const NO_VOLUME_AND_LOCAL_ID_MINOR: u8 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum FileType {
    Thumbnail,
    ChatPhoto,
    Photo,
    Voice,
    Video,
    Document,
    Encrypted,
    Temp,
    Sticker,
    Audio,
    Animation,
    EncryptedThumbnail,
    Wallpaper,
    VideoNote,
    SecureRaw,
    Secure,
    Background,
    DocumentAsFile,
}

const FILE_TYPES: [FileType; 18] = {
    use FileType as T;
    [
        T::Thumbnail,
        T::ChatPhoto,
        T::Photo,
        T::Voice,
        T::Video,
        T::Document,
        T::Encrypted,
        T::Temp,
        T::Sticker,
        T::Audio,
        T::Animation,
        T::EncryptedThumbnail,
        T::Wallpaper,
        T::VideoNote,
        T::SecureRaw,
        T::Secure,
        T::Background,
        T::DocumentAsFile,
    ]
};

impl FileType {
    pub(crate) fn is_photo(self) -> bool {
        use FileType as T;
        matches!(self, T::Thumbnail | T::ChatPhoto | T::Photo | T::Wallpaper | T::EncryptedThumbnail)
    }

    fn from_i32(n: i32) -> Option<FileType> {
        FILE_TYPES.get(usize::try_from(n).ok()?).copied()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ThumbnailSource {
    Legacy { secret: i64 },
    Thumbnail { file_type: FileType, size: char },
    ChatPhotoSmall { chat_id: i64, chat_access_hash: i64 },
    ChatPhotoBig { chat_id: i64, chat_access_hash: i64 },
    StickerSetThumbnail { sticker_set_id: i64, sticker_set_access_hash: i64 },
    FullLegacy { secret: i64 },
    ChatPhotoSmallLegacy { chat_id: i64, chat_access_hash: i64 },
    ChatPhotoBigLegacy { chat_id: i64, chat_access_hash: i64 },
    StickerSetThumbnailLegacy { sticker_set_id: i64, sticker_set_access_hash: i64 },
    StickerSetThumbnailVersion { sticker_set_id: i64, sticker_set_access_hash: i64, sticker_set_version: i32 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileId {
    pub(crate) major: u8,
    pub(crate) minor: u8,
    pub(crate) file_type: FileType,
    pub(crate) dc_id: i32,
    pub(crate) file_reference: Vec<u8>,
    pub(crate) media_id: i64,
    pub(crate) access_hash: i64,
    pub(crate) thumbnail_source: Option<ThumbnailSource>,
    pub(crate) volume_id: i64,
    pub(crate) local_id: i32,
}

impl FileId {
    const MAJOR: u8 = 4;
    const MINOR: u8 = 30;

    pub(crate) fn new(file_type: FileType, dc_id: i32, media_id: i64, access_hash: i64, file_reference: &[u8]) -> Self {
        FileId {
            major: Self::MAJOR,
            minor: Self::MINOR,
            file_type,
            dc_id,
            file_reference: file_reference.into(),
            media_id,
            access_hash,
            thumbnail_source: None,
            volume_id: 0,
            local_id: 0,
        }
    }

    pub(crate) fn decode(file_id: &str) -> Result<FileId, &'static str> {
        let data = unpack(file_id)?;
        let (&major, rest) = data.split_last().ok_or("file id is empty")?;
        let (minor, mut r) = match rest.split_last() {
            Some((&minor, body)) if major >= 4 => (minor, body),
            _ => (0, rest),
        };
        let r = &mut r;
        let (flags, dc_id) = (read::<i32>(r)?, read(r)?);
        if flags & WEB_LOCATION_FLAG != 0 {
            return Err("web file ids aren't supported");
        }
        let file_type = FileType::from_i32(flags & !FILE_REFERENCE_FLAG).ok_or("unknown file type")?;
        let mut id = FileId { major, minor, ..FileId::new(file_type, dc_id, 0, 0, &[]) };
        if flags & FILE_REFERENCE_FLAG != 0 {
            id.file_reference = read(r)?;
        }
        (id.media_id, id.access_hash) = (read(r)?, read(r)?);

        if file_type.is_photo() {
            let has_volume_and_local_id = minor < NO_VOLUME_AND_LOCAL_ID_MINOR;
            let volume_id = if has_volume_and_local_id { read(r)? } else { 0 };
            let source = if major < 4 { 0 } else { read(r)? };
            id.thumbnail_source = Some(id.read_source(source, r)?);
            if has_volume_and_local_id {
                (id.volume_id, id.local_id) = (volume_id, read(r)?);
            }
        }
        Ok(id)
    }

    pub(crate) fn encode(&self) -> String {
        let mut buf = Vec::new();
        let flags = self.file_type as i32;
        let flags = if self.file_reference.is_empty() { flags } else { flags | FILE_REFERENCE_FLAG };
        flags.serialize(&mut buf);
        self.dc_id.serialize(&mut buf);
        if !self.file_reference.is_empty() {
            self.file_reference.serialize(&mut buf);
        }
        self.media_id.serialize(&mut buf);
        self.access_hash.serialize(&mut buf);

        if self.file_type.is_photo() {
            let has_volume_and_local_id = self.minor < NO_VOLUME_AND_LOCAL_ID_MINOR;
            if has_volume_and_local_id {
                self.volume_id.serialize(&mut buf);
            }
            if let Some(source) = &self.thumbnail_source {
                self.write_source(source, &mut buf);
            }
            if has_volume_and_local_id {
                self.local_id.serialize(&mut buf);
            }
        }
        buf.extend_from_slice(&[self.minor, self.major]);
        pack(&buf)
    }

    fn read_source(&mut self, source: i32, r: &mut &[u8]) -> Result<ThumbnailSource, &'static str> {
        use ThumbnailSource as S;
        Ok(match source {
            0 => S::Legacy { secret: read(r)? },
            1 => {
                let file_type = FileType::from_i32(read(r)?).ok_or("unknown thumbnail file type")?;
                let size = u32::try_from(read::<i32>(r)?).ok().and_then(char::from_u32).ok_or("bad thumbnail size")?;
                S::Thumbnail { file_type, size }
            }
            2 => S::ChatPhotoSmall { chat_id: read(r)?, chat_access_hash: read(r)? },
            3 => S::ChatPhotoBig { chat_id: read(r)?, chat_access_hash: read(r)? },
            4 => S::StickerSetThumbnail { sticker_set_id: read(r)?, sticker_set_access_hash: read(r)? },
            5 => {
                self.volume_id = read(r)?;
                let secret = read(r)?;
                self.local_id = read(r)?;
                S::FullLegacy { secret }
            }
            6 | 7 => {
                let (chat_id, chat_access_hash) = (read(r)?, read(r)?);
                (self.volume_id, self.local_id) = (read(r)?, read(r)?);
                if source == 6 {
                    S::ChatPhotoSmallLegacy { chat_id, chat_access_hash }
                } else {
                    S::ChatPhotoBigLegacy { chat_id, chat_access_hash }
                }
            }
            8 => {
                let (sticker_set_id, sticker_set_access_hash) = (read(r)?, read(r)?);
                (self.volume_id, self.local_id) = (read(r)?, read(r)?);
                S::StickerSetThumbnailLegacy { sticker_set_id, sticker_set_access_hash }
            }
            9 => S::StickerSetThumbnailVersion {
                sticker_set_id: read(r)?,
                sticker_set_access_hash: read(r)?,
                sticker_set_version: read(r)?,
            },
            _ => return Err("unknown thumbnail source"),
        })
    }

    fn write_source(&self, source: &ThumbnailSource, buf: &mut Vec<u8>) {
        use ThumbnailSource as S;
        if self.major >= 4 {
            let tag: i32 = match source {
                S::Legacy { .. } => 0,
                S::Thumbnail { .. } => 1,
                S::ChatPhotoSmall { .. } => 2,
                S::ChatPhotoBig { .. } => 3,
                S::StickerSetThumbnail { .. } => 4,
                S::FullLegacy { .. } => 5,
                S::ChatPhotoSmallLegacy { .. } => 6,
                S::ChatPhotoBigLegacy { .. } => 7,
                S::StickerSetThumbnailLegacy { .. } => 8,
                S::StickerSetThumbnailVersion { .. } => 9,
            };
            tag.serialize(buf);
        }
        match *source {
            S::Legacy { secret } => secret.serialize(buf),
            S::Thumbnail { file_type, size } => {
                (file_type as i32).serialize(buf);
                u32::from(size).serialize(buf);
            }
            S::ChatPhotoSmall { chat_id, chat_access_hash } | S::ChatPhotoBig { chat_id, chat_access_hash } => {
                chat_id.serialize(buf);
                chat_access_hash.serialize(buf);
            }
            S::StickerSetThumbnail { sticker_set_id, sticker_set_access_hash } => {
                sticker_set_id.serialize(buf);
                sticker_set_access_hash.serialize(buf);
            }
            S::FullLegacy { secret } => {
                self.volume_id.serialize(buf);
                secret.serialize(buf);
                self.local_id.serialize(buf);
            }
            S::ChatPhotoSmallLegacy { chat_id, chat_access_hash }
            | S::ChatPhotoBigLegacy { chat_id, chat_access_hash } => {
                chat_id.serialize(buf);
                chat_access_hash.serialize(buf);
                self.volume_id.serialize(buf);
                self.local_id.serialize(buf);
            }
            S::StickerSetThumbnailLegacy { sticker_set_id, sticker_set_access_hash } => {
                sticker_set_id.serialize(buf);
                sticker_set_access_hash.serialize(buf);
                self.volume_id.serialize(buf);
                self.local_id.serialize(buf);
            }
            S::StickerSetThumbnailVersion { sticker_set_id, sticker_set_access_hash, sticker_set_version } => {
                sticker_set_id.serialize(buf);
                sticker_set_access_hash.serialize(buf);
                sticker_set_version.serialize(buf);
            }
        }
    }
}

/// The `file_unique_id` of a photo or document, stable across this library's `file_id`s for it. Bot API photo ids
/// use another layout (they start with `AQAD`), so a photo's id won't match one the Bot API gave out.
pub(crate) fn unique_id(media_id: i64) -> String {
    let mut buf = Vec::with_capacity(12);
    2i32.serialize(&mut buf);
    media_id.serialize(&mut buf);
    pack(&buf)
}

fn read<T: Deserializable>(r: &mut &[u8]) -> Result<T, &'static str> {
    T::deserialize(r).map_err(|_| "file id is too short")
}

fn pack(data: &[u8]) -> String {
    let mut out = Vec::with_capacity(data.len());
    let mut zeros = 0u8;
    for &b in data {
        if b == 0 {
            zeros += 1;
            if zeros == u8::MAX {
                out.extend_from_slice(&[0, zeros]);
                zeros = 0;
            }
            continue;
        }
        if zeros > 0 {
            out.extend_from_slice(&[0, zeros]);
            zeros = 0;
        }
        out.push(b);
    }
    if zeros > 0 {
        out.extend_from_slice(&[0, zeros]);
    }
    b64_encode(&out)
}

fn unpack(s: &str) -> Result<Vec<u8>, &'static str> {
    let data = b64_decode(s).ok_or("file id has characters outside base64url")?;
    let (mut out, mut zero) = (Vec::new(), false);
    for b in data {
        if b == 0 {
            zero = true;
        } else if zero {
            out.resize(out.len() + usize::from(b), 0);
            zero = false;
        } else {
            out.push(b);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{FILE_TYPES, FileId, FileType, ThumbnailSource, unique_id};

    #[test]
    fn reads_and_rewrites_a_bot_api_photo_id() {
        let s = "AgACAgIAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAIG9DEbXX3wSq3oI7t_PqQGAQADAgADbQADNgQ";
        let id = FileId::decode(s).unwrap();
        assert_eq!((id.major, id.minor, id.file_type, id.dc_id), (4, 54, FileType::Photo, 2));
        assert_eq!(
            (id.media_id, id.access_hash, id.file_reference.len()),
            (5399953792058913798, 478576178729576621, 25)
        );
        assert_eq!(id.thumbnail_source, Some(ThumbnailSource::Thumbnail { file_type: FileType::Photo, size: 'm' }));
        assert_eq!(id.encode(), s);
        assert_eq!(unique_id(id.media_id), "AgADBvQxG1198Eo");
    }

    #[test]
    fn writes_document_ids() {
        let long: Vec<u8> = (0..300).map(|i| (i % 255 + 1) as u8).collect();
        let mut zeros = vec![0; 302];
        (zeros[0], zeros[301]) = (1, 2);
        let cases = [
            (FileId::new(FileType::Document, 1, 42, 7, &[]), "BQADAQADKgAHBwAHHgQ"),
            (FileId::new(FileType::Video, 5, 1, 2, &long), VIDEO_LONG_REFERENCE),
            (FileId::new(FileType::Document, 2, 3, 4, &zeros), "BQACAgIAA_4uAQABAQD_AC0CAAIDAAcEAAceBA"),
        ];
        for (id, s) in cases {
            assert_eq!((id.encode().as_str(), FileId::decode(s).as_ref()), (s, Ok(&id)));
        }
        // Some encoders add a minor/major pair to documents without a file reference; it is read past.
        let padded = FileId::decode("BQADAQADKgAHBwAHHgADBAADHgQ");
        assert_eq!(padded, Ok(FileId::new(FileType::Document, 1, 42, 7, &[])));
        assert!(FILE_TYPES.iter().enumerate().all(|(i, &t)| t as usize == i));
    }

    #[test]
    fn every_thumbnail_source_round_trips() {
        use ThumbnailSource as S;
        let sources = [
            S::Legacy { secret: -55 },
            S::Thumbnail { file_type: FileType::Sticker, size: 's' },
            S::ChatPhotoSmall { chat_id: 777, chat_access_hash: -888 },
            S::StickerSetThumbnail { sticker_set_id: 99, sticker_set_access_hash: -100 },
            S::FullLegacy { secret: -55 },
            S::ChatPhotoBigLegacy { chat_id: -5, chat_access_hash: 0 },
            S::StickerSetThumbnailLegacy { sticker_set_id: 99, sticker_set_access_hash: -100 },
            S::StickerSetThumbnailVersion { sticker_set_id: 99, sticker_set_access_hash: -100, sticker_set_version: 3 },
        ];
        let at_30 = [
            "AAMCAwADAgUGAAELAAfq_________9__________AATJ_________ywAAx4E",
            "AAMCAwADAgUGAAELAAfq_________9__________AQADCAADcwADLAADHgQ",
            "AAMCAwADAgUGAAELAAfq_________9__________AgADCQMABoj8________LAADHgQ",
            "AAMCAwADAgUGAAELAAfq_________9__________BAADYwAHnP________8sAAMeBA",
            "AAMCAwADAgUGAAELAAfq_________9__________BQAD3__________J_________ywAAywAAx4E",
            "AAMCAwADAgUGAAELAAfq_________9__________BwAD-_________8ACN__________LAADLAADHgQ",
            "AAMCAwADAgUGAAELAAfq_________9__________CAADYwAHnP_________f_________ywAAywAAx4E",
            "AAMCAwADAgUGAAELAAfq_________9__________CQADYwAHnP________8DAAMsAAMeBA",
        ];
        let at_54 = [
            "AAMCAwADAgUGAAELAAfq_________wAEyf________82BA",
            "AAMCAwADAgUGAAELAAfq_________wEAAwgAA3MAAzYE",
            "AAMCAwADAgUGAAELAAfq_________wIAAwkDAAaI_P_______zYE",
            "AAMCAwADAgUGAAELAAfq_________wQAA2MAB5z_________NgQ",
            "AAMCAwADAgUGAAELAAfq_________wUAA9__________yf________8sAAM2BA",
            "AAMCAwADAgUGAAELAAfq_________wcAA_v_________AAjf_________ywAAzYE",
            "AAMCAwADAgUGAAELAAfq_________wgAA2MAB5z_________3_________8sAAM2BA",
            "AAMCAwADAgUGAAELAAfq_________wkAA2MAB5z_________AwADNgQ",
        ];
        for (i, source) in sources.into_iter().enumerate() {
            let legacy = matches!(source, S::FullLegacy { .. } | S::ChatPhotoBigLegacy { .. });
            let legacy = legacy || matches!(source, S::StickerSetThumbnailLegacy { .. });
            for (minor, s) in [(30, at_30[i]), (54, at_54[i])] {
                let (volume_id, local_id) = if minor == 30 || legacy { (-33, 44) } else { (0, 0) };
                let base = FileId::new(FileType::Thumbnail, 3, 11, -22, &[5, 6]);
                let id = FileId { minor, thumbnail_source: Some(source), volume_id, local_id, ..base };
                assert_eq!((id.encode().as_str(), FileId::decode(s).as_ref()), (s, Ok(&id)), "{source:?} at {minor}");
            }
        }
    }

    #[test]
    fn reads_old_ids() {
        let old = FileId::decode("AgADAgADAQAHAgAHAwAHBQAHBAAEAg").unwrap();
        assert_eq!(
            (old.major, old.minor, old.media_id, old.access_hash, old.volume_id, old.local_id),
            (2, 0, 1, 2, 3, 4)
        );
        assert_eq!(old.thumbnail_source, Some(ThumbnailSource::Legacy { secret: 5 }));
        assert_eq!(old.encode(), "AgADAgADAQAHAgAHAwAHBQAHBAAEAg");
    }

    #[test]
    fn rejects_broken_ids() {
        let cases = [
            ("", "file id is empty"),
            ("not*base64", "file id has characters outside base64url"),
            ("BQADAQADKgAHBw", "file id is too short"),
            ("EgADAQADKgAHBwAHHgQ", "unknown file type"),
            ("AgACAQQAAxlodHRwczovL2V4YW1wbGUuY29tL2EucG5nAAJjAAceBA", "web file ids aren't supported"),
        ];
        for (s, why) in cases {
            assert_eq!(FileId::decode(s), Err(why), "{s}");
        }
    }

    const VIDEO_LONG_REFERENCE: &str = concat!(
        "BAACAgUAA_4sAQABAQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyAhIiMkJSYnKCkqKywtLi8wMTIzNDU2Nzg5Ojs8PT4_QEFCQ0RFRk",
        "dISUpLTE1OT1BRUlNUVVZXWFlaW1xdXl9gYWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXp7fH1-f4CBgoOEhYaHiImKi4yNjo-QkZKTlJWWl5iZ",
        "mpucnZ6foKGio6SlpqeoqaqrrK2ur7CxsrO0tba3uLm6u7y9vr_AwcLDxMXGx8jJysvMzc7P0NHS09TV1tfY2drb3N3e3-Dh4uPk5ebn6Onq6-",
        "zt7u_w8fLz9PX29_j5-vv8_f7_AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyAhIiMkJSYnKCkqKywtAQAHAgAHHgQ",
    );
}
