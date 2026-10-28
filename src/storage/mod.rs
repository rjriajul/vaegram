pub mod session_string;
pub mod state;

use std::fs;
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::Path;

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
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&tmp)?;
    file.write_all(bytes)?;
    // Without this, a power loss after the rename can leave an empty file on ext4 and xfs.
    file.sync_all()?;
    // `mode` only applies when the temp file is created; a leftover one could be world-readable.
    #[cfg(unix)]
    file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    fs::rename(&tmp, path)
}

pub fn delete(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}
