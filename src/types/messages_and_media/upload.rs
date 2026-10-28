use std::path::{Path, PathBuf};

const MIME_TYPES: &[(&str, &str)] = &[
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("png", "image/png"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("heic", "image/heic"),
    ("avif", "image/avif"),
    ("bmp", "image/bmp"),
    ("tif", "image/tiff"),
    ("tiff", "image/tiff"),
    ("svg", "image/svg+xml"),
    ("mp4", "video/mp4"),
    ("m4v", "video/x-m4v"),
    ("mov", "video/quicktime"),
    ("mkv", "video/x-matroska"),
    ("webm", "video/webm"),
    ("avi", "video/x-msvideo"),
    ("3gp", "video/3gpp"),
    ("mpeg", "video/mpeg"),
    ("mp3", "audio/mpeg"),
    ("m4a", "audio/mp4"),
    ("aac", "audio/aac"),
    ("ogg", "audio/ogg"),
    ("oga", "audio/ogg"),
    ("opus", "audio/ogg"),
    ("flac", "audio/flac"),
    ("wav", "audio/wav"),
    ("pdf", "application/pdf"),
    ("zip", "application/zip"),
    ("rar", "application/vnd.rar"),
    ("7z", "application/x-7z-compressed"),
    ("gz", "application/gzip"),
    ("tar", "application/x-tar"),
    ("apk", "application/vnd.android.package-archive"),
    ("epub", "application/epub+zip"),
    ("json", "application/json"),
    ("xml", "application/xml"),
    ("doc", "application/msword"),
    ("docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
    ("xls", "application/vnd.ms-excel"),
    ("xlsx", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
    ("ppt", "application/vnd.ms-powerpoint"),
    ("pptx", "application/vnd.openxmlformats-officedocument.presentationml.presentation"),
    ("tgs", "application/x-tgsticker"),
    ("txt", "text/plain"),
    ("md", "text/markdown"),
    ("csv", "text/csv"),
    ("html", "text/html"),
    ("htm", "text/html"),
    ("css", "text/css"),
    ("js", "text/javascript"),
];

#[derive(Clone, Copy, Debug)]
pub enum Upload<'a> {
    Path(&'a Path),
    Bytes { name: &'a str, data: &'a [u8] },
}

impl<'a> Upload<'a> {
    pub fn bytes(name: &'a str, data: &'a [u8]) -> Self {
        Upload::Bytes { name, data }
    }

    pub fn name(&self) -> &'a str {
        match self {
            Upload::Path(path) => path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
            Upload::Bytes { name, .. } => name,
        }
    }

    pub(crate) fn mime_type(&self) -> &'static str {
        let extension = self.name().rsplit_once('.').map_or("", |(_, e)| e);
        let known = MIME_TYPES.iter().find(|(e, _)| e.eq_ignore_ascii_case(extension));
        known.map_or("application/octet-stream", |m| m.1)
    }
}

impl<'a> From<&'a Path> for Upload<'a> {
    fn from(path: &'a Path) -> Self {
        Upload::Path(path)
    }
}

impl<'a> From<&'a PathBuf> for Upload<'a> {
    fn from(path: &'a PathBuf) -> Self {
        Upload::Path(path)
    }
}

impl<'a> From<&'a str> for Upload<'a> {
    fn from(path: &'a str) -> Self {
        Upload::Path(Path::new(path))
    }
}

#[cfg(test)]
mod tests {
    use super::Upload;

    #[test]
    fn guesses_the_mime_type_from_the_extension() {
        let mime = |name: &str| Upload::bytes(name, b"").mime_type();
        assert_eq!((mime("a.jpg"), mime("A.MP4"), mime("a.tar.gz")), ("image/jpeg", "video/mp4", "application/gzip"));
        assert_eq!([mime("a.bin"), mime("README"), mime("a.")], ["application/octet-stream"; 3]);
    }
}
