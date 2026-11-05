use crate::file_id::{FileId, FileType, ThumbnailSource, thumbnail_unique_id};
use crate::raw::{base, types};
use crate::utils::expand_inline_bytes;

/// A smaller copy of a photo or of a file's preview. Download it by passing its `file_id` to `download_media`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thumbnail {
    /// Identifier to pass to `download_media`.
    pub file_id: String,
    /// Differs per size, unlike the file's own `file_unique_id`.
    pub file_unique_id: String,
    /// Width in pixels.
    pub width: i32,
    /// Height in pixels.
    pub height: i32,
    /// Size of the thumbnail file in bytes.
    pub file_size: i64,
}

/// A few hundred bytes of JPEG data without its header, sent along with media that isn't downloaded yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrippedThumbnail {
    /// The stripped JPEG bytes. Use [`StrippedThumbnail::to_jpeg`] for a viewable image.
    pub data: Vec<u8>,
}

impl StrippedThumbnail {
    /// The thumbnail as a complete JPEG. Empty when `data` isn't a stripped thumbnail.
    /// Rebuilds the full JPEG from the stripped data.
    pub fn to_jpeg(&self) -> Vec<u8> {
        expand_inline_bytes(&self.data).unwrap_or_default()
    }
}

/// `(size letter, width, height, bytes)` of a size Telegram can serve as a file.
fn read_size(size: &base::PhotoSize) -> Option<(&str, i32, i32, i64)> {
    match size {
        base::PhotoSize::PhotoSize(s) => Some((s.r#type.as_str(), s.w, s.h, i64::from(s.size))),
        base::PhotoSize::PhotoSizeProgressive(s) => {
            Some((s.r#type.as_str(), s.w, s.h, i64::from(*s.sizes.iter().max()?)))
        }
        base::PhotoSize::PhotoCachedSize(s) => Some((s.r#type.as_str(), s.w, s.h, s.bytes.len() as i64)),
        _ => None,
    }
}

fn sizes(sizes: &[base::PhotoSize]) -> impl Iterator<Item = (&str, i32, i32, i64)> {
    sizes.iter().filter_map(read_size)
}

/// The size a photo stands for: the one with the most bytes among the sizes that are separate files. Photos and
/// their thumbnails share this choice, so a thumbnail is never the photo's own size.
pub(super) fn full_size(photo_sizes: &[base::PhotoSize]) -> Option<(&str, i32, i32, i64)> {
    let files = photo_sizes.iter().filter(|s| !matches!(s, base::PhotoSize::PhotoCachedSize(_)));
    files.filter_map(read_size).max_by_key(|s| s.3)
}

impl Thumbnail {
    /// Every size of the photo except the one the photo itself stands for.
    pub(crate) fn from_photo(photo: &types::Photo) -> Vec<Thumbnail> {
        let full = full_size(&photo.sizes).map(|s| s.0);
        sizes(&photo.sizes)
            .filter(|size| Some(size.0) != full)
            .filter_map(|size| {
                let id = FileId::new(FileType::Photo, photo.dc_id, photo.id, photo.access_hash, &photo.file_reference);
                Thumbnail::build(id, photo.id, size, FileType::Photo)
            })
            .collect()
    }

    /// Every preview size of the document.
    pub(crate) fn from_document(document: &types::Document) -> Vec<Thumbnail> {
        let sizes = sizes(document.thumbs.as_deref().unwrap_or_default());
        sizes
            .filter_map(|size| {
                let id = FileId::new(
                    FileType::Thumbnail,
                    document.dc_id,
                    document.id,
                    document.access_hash,
                    &document.file_reference,
                );
                Thumbnail::build(id, document.id, size, FileType::Thumbnail)
            })
            .collect()
    }

    fn build(id: FileId, media_id: i64, size: (&str, i32, i32, i64), source_type: FileType) -> Option<Thumbnail> {
        let (letter, width, height, file_size) = size;
        let letter = letter.chars().next()?;
        let source = ThumbnailSource::Thumbnail { file_type: source_type, size: letter };
        let id = FileId { thumbnail_source: Some(source), ..id };
        Some(Thumbnail {
            file_id: id.encode(),
            file_unique_id: thumbnail_unique_id(media_id, letter),
            width,
            height,
            file_size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{StrippedThumbnail, Thumbnail};
    use crate::raw::{base, types};
    use crate::types::{FileLocation, Photo};

    fn size(letter: &str, w: i32, h: i32, bytes: i32) -> base::PhotoSize {
        base::PhotoSize::PhotoSize(Box::new(types::PhotoSize { r#type: letter.into(), w, h, size: bytes }))
    }

    fn photo() -> types::Photo {
        types::Photo {
            has_stickers: false,
            id: 7,
            access_hash: 8,
            file_reference: vec![9],
            date: 0,
            sizes: vec![
                size("s", 90, 60, 1_000),
                size("m", 320, 213, 9_000),
                base::PhotoSize::PhotoStrippedSize(Box::new(types::PhotoStrippedSize {
                    r#type: "i".into(),
                    bytes: vec![1, 2, 3],
                })),
                size("x", 1280, 853, 90_000),
            ],
            video_sizes: None,
            dc_id: 2,
        }
    }

    fn document() -> types::Document {
        types::Document {
            id: 5,
            access_hash: 6,
            file_reference: vec![3],
            date: 0,
            mime_type: "video/mp4".into(),
            size: 100,
            thumbs: Some(vec![size("m", 320, 180, 4_000), size("s", 90, 50, 700)]),
            video_thumbs: None,
            dc_id: 4,
            attributes: Vec::new(),
        }
    }

    #[test]
    fn photo_thumbs_skip_the_full_size() {
        let thumbs = Thumbnail::from_photo(&photo());
        assert_eq!(
            thumbs.iter().map(|t| (t.width, t.height, t.file_size)).collect::<Vec<_>>(),
            [(90, 60, 1_000), (320, 213, 9_000)]
        );
        assert_ne!(thumbs[0].file_unique_id, thumbs[1].file_unique_id, "every size has its own unique id");
        assert!(Thumbnail::from_photo(&types::Photo { sizes: vec![size("x", 1, 1, 1)], ..photo() }).is_empty());
    }

    fn thumb_sizes(thumbs: &[Thumbnail]) -> Vec<String> {
        thumbs
            .iter()
            .map(|t| match FileLocation::try_from(t.file_id.as_str()).unwrap().location {
                base::InputFileLocation::InputPhotoFileLocation(l) => l.thumb_size,
                other => panic!("expected a photo location, got {other:?}"),
            })
            .collect()
    }

    #[test]
    fn photo_thumbs_download_with_their_size_letter() {
        let thumbs = Thumbnail::from_photo(&photo());
        assert_eq!(thumb_sizes(&thumbs), ["s", "m"]);
        let base::InputFileLocation::InputPhotoFileLocation(l) =
            FileLocation::try_from(thumbs[1].file_id.as_str()).unwrap().location
        else {
            panic!("expected a photo location")
        };
        assert_eq!((l.id, l.access_hash, l.file_reference), (7, 8, vec![9]));
    }

    #[test]
    fn thumbs_leave_out_the_size_the_photo_stands_for() {
        // The photo is its biggest file: "m" has the most bytes though "x" covers the most pixels.
        let raw = types::Photo {
            sizes: vec![size("s", 90, 60, 1_000), size("m", 320, 213, 90_000), size("x", 1280, 853, 50_000)],
            ..photo()
        };
        let photo = Photo::parse(&base::Photo::Photo(Box::new(raw.clone()))).unwrap();
        assert_eq!((photo.width, photo.height), (320, 213));
        assert_eq!(thumb_sizes(&Thumbnail::from_photo(&raw)), ["s", "x"]);
        assert_eq!(thumb_sizes(&photo.thumbs), ["s", "x"]);
    }

    #[test]
    fn document_thumbs_download_with_thumb_size() {
        let thumbs = Thumbnail::from_document(&document());
        assert_eq!(thumbs.len(), 2);
        let base::InputFileLocation::InputDocumentFileLocation(l) =
            FileLocation::try_from(thumbs[0].file_id.as_str()).unwrap().location
        else {
            panic!("expected a document location")
        };
        assert_eq!((l.id, l.access_hash, l.thumb_size.as_str()), (5, 6, "m"));
        let base::InputFileLocation::InputDocumentFileLocation(l) =
            FileLocation::try_from(thumbs[1].file_id.as_str()).unwrap().location
        else {
            panic!("expected a document location")
        };
        assert_eq!(l.thumb_size, "s");
        assert!(Thumbnail::from_document(&types::Document { thumbs: None, ..document() }).is_empty());
    }

    #[test]
    fn stripped_bytes_expand_to_a_jpeg() {
        let stripped = StrippedThumbnail { data: vec![1, 0x0e, 0x28, 0xa3, 0x9e] };
        let jpeg = stripped.to_jpeg();
        assert_eq!(&jpeg[..2], &[0xff, 0xd8]);
        assert_eq!((jpeg[164], jpeg[166]), (0x0e, 0x28));
        assert_eq!(&jpeg[jpeg.len() - 2..], &[0xff, 0xd9]);
        assert!(StrippedThumbnail { data: vec![1, 2] }.to_jpeg().is_empty());
        assert!(StrippedThumbnail { data: vec![2, 0x0e, 0x28, 1] }.to_jpeg().is_empty());
    }
}
