pub mod session_string;
pub mod state;

use std::fs;
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::Path;

use crate::crypto::random;

pub struct SessionData {
    pub dc_id: i32,
    pub test_mode: bool,
    pub addr: Option<SocketAddr>,
    pub api_id: i32,
    pub auth_key: [u8; 256],
    pub user_id: i64,
    pub is_bot: bool,
}

pub fn load(path: &Path) -> io::Result<Option<SessionData>> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    session_string::parse(&text)
        .map(Some)
        .map_err(|why| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {why}", path.display())))
}

pub fn save(path: &Path, data: &SessionData) -> io::Result<()> {
    write_private(path, session_string::export(data).as_bytes())
}

pub fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    // A fresh name opened with create_new never reuses a file or follows a link someone left at a guessable name,
    // and the 0600 mode is set when it is created, before the key is in it.
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{:016x}.tmp", u64::from_le_bytes(random())));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&tmp)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    // Without the sync, a power loss after the rename can leave an empty file on ext4 and xfs.
    let result = written.and_then(|()| fs::rename(&tmp, path));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

pub fn delete(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::write_private;

    #[test]
    fn leaves_a_file_at_the_old_temp_name_alone() {
        let dir = std::env::temp_dir().join(format!("vaegram-write-private-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (path, planted) = (dir.join("bot.session"), dir.join("bot.session.tmp"));
        std::fs::write(&planted, b"planted").unwrap();
        let written = write_private(&path, b"key");
        let (saved, left) = (std::fs::read(&path), std::fs::read(&planted));
        let leftovers = std::fs::read_dir(&dir).unwrap().count();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(written.is_ok(), "{written:?}");
        assert_eq!((saved.unwrap(), left.ok(), leftovers), (b"key".to_vec(), Some(b"planted".to_vec()), 2));
    }

    #[test]
    fn a_failed_save_leaves_no_temp_file() {
        let dir = std::env::temp_dir().join(format!("vaegram-write-private-fail-{}", std::process::id()));
        // A directory where the file should go makes the final rename fail.
        let path = dir.join("bot.session");
        std::fs::create_dir_all(path.join("inside")).unwrap();
        let written = write_private(&path, b"key");
        let names: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(written.is_err());
        assert_eq!(names, ["bot.session"]);
    }
}
