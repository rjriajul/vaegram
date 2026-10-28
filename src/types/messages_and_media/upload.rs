use std::path::{Path, PathBuf};

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
