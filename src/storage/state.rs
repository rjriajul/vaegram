//! `<name>.state`, kept next to `<name>.session`: access hashes and the update state, so a restart
//! can still reach known chats and fetch what it missed with `updates.getDifference`.
//!
//! Little-endian: version 2 (u8), owner user_id (i64), server salt and clock offset (i64, not in version 1),
//! pts, qts, date, seq (i32, date 0 means none),
//! channel count (u32) then channel_id (i64) and pts (i32) each, peer count (u32) then chat_id (i64)
//! and access_hash (i64) each. Chat ids are Bot API style, so the sign tells users from channels.
//! Basic groups need no hash and aren't stored.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use crate::raw::{base, types};
use crate::session::Salt;
use crate::utils::{CHANNEL_OFFSET, get_channel_id};

const VERSION: u8 = 2;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpdateState {
    pub pts: i32,
    pub qts: i32,
    pub date: i32,
    pub seq: i32,
    pub channels: HashMap<i64, i32>,
}

pub type Peers = HashMap<i64, base::InputPeer>;

pub fn encode(user_id: i64, salt: Salt, peers: &Peers, state: Option<&UpdateState>) -> Vec<u8> {
    let none = UpdateState::default();
    let s = state.unwrap_or(&none);
    let mut b = Vec::with_capacity(49 + s.channels.len() * 12 + peers.len() * 16);
    b.push(VERSION);
    for n in [user_id, salt.value, salt.time_offset] {
        b.extend_from_slice(&n.to_le_bytes());
    }
    for n in [s.pts, s.qts, s.date, s.seq] {
        b.extend_from_slice(&n.to_le_bytes());
    }
    b.extend_from_slice(&(s.channels.len() as u32).to_le_bytes());
    for (id, pts) in &s.channels {
        b.extend_from_slice(&id.to_le_bytes());
        b.extend_from_slice(&pts.to_le_bytes());
    }
    let count_at = b.len();
    b.extend_from_slice(&[0; 4]);
    let mut count = 0u32;
    for peer in peers.values() {
        let (id, hash) = match peer {
            base::InputPeer::InputPeerUser(p) => (p.user_id, p.access_hash),
            base::InputPeer::InputPeerChannel(p) => (get_channel_id(p.channel_id), p.access_hash),
            _ => continue,
        };
        b.extend_from_slice(&id.to_le_bytes());
        b.extend_from_slice(&hash.to_le_bytes());
        count += 1;
    }
    b[count_at..count_at + 4].copy_from_slice(&count.to_le_bytes());
    b
}

pub fn decode(mut b: &[u8], user_id: i64) -> Option<(Peers, Option<UpdateState>, Salt)> {
    let [version] = take::<1>(&mut b)?;
    if !(1..=VERSION).contains(&version) {
        return None;
    }
    let owner = i64::from_le_bytes(take(&mut b)?);
    let salt = match version {
        1 => Salt::default(),
        _ => Salt { value: i64::from_le_bytes(take(&mut b)?), time_offset: i64::from_le_bytes(take(&mut b)?) },
    };
    let mut i32s = [0; 4];
    for n in &mut i32s {
        *n = i32::from_le_bytes(take(&mut b)?);
    }
    let [pts, qts, date, seq] = i32s;
    let count = u32::from_le_bytes(take(&mut b)?) as usize;
    let mut channels = HashMap::with_capacity(count.min(b.len() / 12));
    for _ in 0..count {
        channels.insert(i64::from_le_bytes(take(&mut b)?), i32::from_le_bytes(take(&mut b)?));
    }
    let count = u32::from_le_bytes(take(&mut b)?) as usize;
    let mut peers = Peers::with_capacity(count.min(b.len() / 16));
    for _ in 0..count {
        let (id, access_hash) = (i64::from_le_bytes(take(&mut b)?), i64::from_le_bytes(take(&mut b)?));
        let peer = if id > 0 {
            base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: id, access_hash }))
        } else if id <= -CHANNEL_OFFSET {
            let channel_id = id.checked_neg()?.checked_sub(CHANNEL_OFFSET)?;
            base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id, access_hash }))
        } else {
            return None;
        };
        peers.insert(id, peer);
    }
    if !b.is_empty() {
        return None;
    }
    if owner != user_id {
        return Some((Peers::new(), None, Salt::default()));
    }
    let state = (date != 0).then_some(UpdateState { pts, qts, date, seq, channels });
    Some((peers, state, salt))
}

pub fn load(path: &Path, user_id: i64) -> io::Result<(Peers, Option<UpdateState>, Salt)> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Default::default()),
        Err(e) => return Err(e),
    };
    Ok(decode(&bytes, user_id).unwrap_or_else(|| {
        eprintln!("vaegram: ignoring {}: damaged or from a newer vaegram; it will be overwritten", path.display());
        Default::default()
    }))
}

pub(crate) struct StateFile {
    pub(crate) path: PathBuf,
    encoded: AtomicU64,
    written: Mutex<u64>,
}

impl StateFile {
    pub(crate) fn new(path: PathBuf) -> Self {
        StateFile { path, encoded: AtomicU64::new(0), written: Mutex::new(0) }
    }

    pub(crate) fn ticket(&self) -> u64 {
        self.encoded.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub(crate) fn write(&self, ticket: u64, bytes: &[u8]) {
        let mut written = self.written.lock().unwrap_or_else(PoisonError::into_inner);
        if ticket < *written {
            return;
        }
        match super::write_private(&self.path, bytes) {
            Ok(()) => *written = ticket,
            Err(e) => eprintln!("vaegram: failed to save {}: {e}", self.path.display()),
        }
    }

    /// Removes the file for good: no later write, even one already queued, recreates it.
    pub(crate) fn delete(&self) -> io::Result<()> {
        *self.written.lock().unwrap_or_else(PoisonError::into_inner) = u64::MAX;
        super::delete(&self.path)
    }
}

fn take<const N: usize>(b: &mut &[u8]) -> Option<[u8; N]> {
    let (head, rest) = b.split_first_chunk::<N>()?;
    *b = rest;
    Some(*head)
}

#[cfg(test)]
mod tests {
    use super::{Peers, Salt, StateFile, decode, encode};

    #[test]
    fn min_peer_id_is_damage_not_a_panic() {
        let mut b = encode(1, Salt::default(), &Peers::new(), None);
        let at = b.len() - 4;
        b[at..].copy_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&i64::MIN.to_le_bytes());
        b.extend_from_slice(&0i64.to_le_bytes());
        assert!(decode(&b, 1).is_none());
    }

    #[test]
    fn older_snapshot_never_replaces_newer() {
        let path = std::env::temp_dir().join(format!("vaegram-order-{}.state", std::process::id()));
        let file = StateFile::new(path.clone());
        let (old, new) = (file.ticket(), file.ticket());
        file.write(new, b"new");
        file.write(old, b"old");
        let saved = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(saved, b"new");
    }
}
