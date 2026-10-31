use crate::raw::base;

/// A file uploaded with [`upload_file`](crate::Client::upload_file) or
/// [`upload_stream`](crate::Client::upload_stream); pass `&file` to any media sender.
///
/// Telegram keeps the parts "between several minutes and several hours"
/// (<https://core.telegram.org/api/files#uploading-files>), then a send fails with `Error::Rpc`
/// (`FILE_PART_…_MISSING`). Send it soon; for longer reuse, resend the sent message's media.
#[cfg_attr(
    not(feature = "unstable-raw"),
    doc = r#"
Its TL file is only reachable with the `unstable-raw` feature:

```compile_fail
fn raw(file: vaegram::types::UploadedFile) {
    let _ = file.into_raw();
}
```
"#
)]
#[derive(Debug, Clone)]
pub struct UploadedFile(pub(crate) base::InputFile);

impl UploadedFile {
    pub fn name(&self) -> &str {
        match &self.0 {
            base::InputFile::InputFile(f) => &f.name,
            base::InputFile::InputFileBig(f) => &f.name,
            _ => "",
        }
    }

    /// Experimental (`unstable-raw`): may change or go away in any release.
    #[cfg(feature = "unstable-raw")]
    pub fn into_raw(self) -> base::InputFile {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::UploadedFile;
    use crate::raw::{base, types};

    fn small() -> base::InputFile {
        let file = types::InputFile { id: 7, parts: 1, name: "a.jpg".into(), md5_checksum: String::new() };
        base::InputFile::InputFile(Box::new(file))
    }

    #[test]
    fn name_reads_small_and_big_files() {
        let big = types::InputFileBig { id: 7, parts: 21, name: "b.mp4".into() };
        let big = UploadedFile(base::InputFile::InputFileBig(Box::new(big)));
        assert_eq!((UploadedFile(small()).name(), big.name()), ("a.jpg", "b.mp4"));
    }

    #[cfg(feature = "unstable-raw")]
    #[test]
    fn into_raw_gives_the_uploaded_input_file() {
        assert_eq!(UploadedFile(small()).into_raw(), small());
    }
}
