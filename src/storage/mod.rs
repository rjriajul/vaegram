pub mod session_string;

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
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(session_string::export(data).as_bytes())
}
