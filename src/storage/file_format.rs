//! The session file: a 20-byte header (`vaegram-session\0`, format u16 = 1, flags u16), then records appended
//! one after another. Each record is `len: u32 | crc32: u32 | kind: u8 | body`, little-endian, where `len`
//! counts the kind and the body and the CRC covers both.
//!
//! | Kind | Record      | Body                                                                                   |
//! |------|-------------|----------------------------------------------------------------------------------------|
//! | 0x01 | session     | the generation byte and fields of a session string                                     |
//! | 0x02 | state       | owner i64, salt i64, clock offset i64, pts/qts/date/seq i32 (date 0: none), channel count u32, then channel i64 + pts i32 each |
//! | 0x03 | peer        | id i64, access_hash i64, updated i64, phone (u8 length + bytes, 0: none), username count u8, then u8 length + bytes each |
//! | 0x04 | clear peers | empty                                                                                  |
//!
//! Replaying keeps the last session and state, upserts peers, and drops the peers before each clear. Unknown kinds
//! below 0x80 are skipped but kept whole through compaction; from 0x80 up the file needs a newer vaegram. A short
//! record or one whose CRC fails ends the replay: everything after it is a torn write. A record whose CRC holds
//! but whose body won't decode refuses the whole file.

use std::collections::HashMap;
use std::io;

use super::engine::{PeerRecord, StoredState};
use super::memory_storage::Tables;
use super::{UpdateState, session_string};
use crate::session::Salt;

const MAGIC: &[u8; 16] = b"vaegram-session\0";
const FORMAT: u16 = 1;
pub(crate) const HEADER_LEN: usize = 20;
const SQLITE_MAGIC: &[u8; 16] = b"SQLite format 3\0";

const SESSION: u8 = 0x01;
const STATE: u8 = 0x02;
const PEER: u8 = 0x03;
const CLEAR_PEERS: u8 = 0x04;
const CRITICAL: u8 = 0x80;

const NEWER: &str = "this session file needs a newer vaegram";
const SQLITE: &str =
    "a SQLite session from another library: open it with SqliteStorage (feature `sqlite`) or import its session string";
const FOREIGN: &str =
    "not a vaegram session file (an older vaegram wrote session strings here): delete it and log in again";
const MALFORMED: &str = "a record in this session file is malformed; the file was left as it is";

pub(crate) fn header() -> [u8; HEADER_LEN] {
    let mut h = [0; HEADER_LEN];
    h[..16].copy_from_slice(MAGIC);
    h[16..18].copy_from_slice(&FORMAT.to_le_bytes());
    h
}

fn record(out: &mut Vec<u8>, kind: u8, body: impl FnOnce(&mut Vec<u8>)) {
    let start = out.len();
    out.extend_from_slice(&[0; 8]);
    out.push(kind);
    body(out);
    let len = (out.len() - start - 8) as u32;
    let crc = crc32fast::hash(&out[start + 8..]);
    out[start..start + 4].copy_from_slice(&len.to_le_bytes());
    out[start + 4..start + 8].copy_from_slice(&crc.to_le_bytes());
}

pub(crate) fn session(out: &mut Vec<u8>, session: &super::SessionData) {
    record(out, SESSION, |b| b.extend_from_slice(&session_string::encode_fields(session)));
}

pub(crate) fn state(out: &mut Vec<u8>, state: &StoredState) {
    record(out, STATE, |b| {
        let none = UpdateState::default();
        let update = state.update.as_ref().unwrap_or(&none);
        for n in [state.owner, state.salt.value, state.salt.time_offset] {
            b.extend_from_slice(&n.to_le_bytes());
        }
        for n in [update.pts, update.qts, update.date, update.seq] {
            b.extend_from_slice(&n.to_le_bytes());
        }
        b.extend_from_slice(&(update.channels.len() as u32).to_le_bytes());
        for (id, pts) in &update.channels {
            b.extend_from_slice(&id.to_le_bytes());
            b.extend_from_slice(&pts.to_le_bytes());
        }
    });
}

pub(crate) fn peer(out: &mut Vec<u8>, peer: &PeerRecord) {
    record(out, PEER, |b| {
        for n in [peer.id, peer.access_hash, peer.updated] {
            b.extend_from_slice(&n.to_le_bytes());
        }
        // Telegram's usernames and numbers are far shorter than a length byte allows; longer ones aren't real.
        let short = |s: &&String| s.len() <= u8::MAX as usize;
        let phone = peer.phone_number.as_ref().filter(short).map_or(&[][..], |p| p.as_bytes());
        b.push(phone.len() as u8);
        b.extend_from_slice(phone);
        let count_at = b.len();
        b.push(0);
        for username in peer.usernames.iter().filter(short).take(u8::MAX as usize) {
            b[count_at] += 1;
            b.push(username.len() as u8);
            b.extend_from_slice(username.as_bytes());
        }
    });
}

pub(crate) fn clear_peers(out: &mut Vec<u8>) {
    record(out, CLEAR_PEERS, |_| {});
}

/// The header and the records that rebuild `tables`, as compaction writes them. `skipped` holds the whole records
/// of kinds this version doesn't know, which a newer vaegram may still want.
pub(crate) fn snapshot(tables: &Tables, skipped: &[u8]) -> Vec<u8> {
    let mut out = header().to_vec();
    if let Some(s) = &tables.session {
        session(&mut out, s);
    }
    if let Some(s) = &tables.state {
        state(&mut out, s);
    }
    for p in tables.peers_oldest_first() {
        peer(&mut out, p);
    }
    out.extend_from_slice(skipped);
    out
}

pub(crate) struct Replayed {
    pub(crate) tables: Tables,
    /// The whole records of skipped kinds, in file order.
    pub(crate) skipped: Vec<u8>,
    /// Where the last whole record ends: a write starts there.
    pub(crate) good_len: usize,
    /// Whether bytes after `good_len` were dropped.
    pub(crate) damaged: bool,
}

/// Rebuilds the tables from a session file. An empty or half-written header is a new file.
pub(crate) fn replay(bytes: &[u8]) -> io::Result<Replayed> {
    let refuse = |why: &str| Err(io::Error::new(io::ErrorKind::InvalidData, why.to_owned()));
    let mut replayed = Replayed { tables: Tables::default(), skipped: Vec::new(), good_len: 0, damaged: false };
    if bytes.len() < HEADER_LEN && header().starts_with(bytes) {
        replayed.damaged = !bytes.is_empty();
        return Ok(replayed);
    }
    if bytes.starts_with(SQLITE_MAGIC) {
        return refuse(SQLITE);
    }
    if bytes.len() < HEADER_LEN || !bytes.starts_with(MAGIC) {
        return refuse(FOREIGN);
    }
    match u16::from_le_bytes([bytes[16], bytes[17]]) {
        FORMAT => {}
        0 => return refuse(FOREIGN),
        _ => return refuse(NEWER),
    }
    let mut at = HEADER_LEN;
    replayed.good_len = at;
    while at < bytes.len() {
        let Some((kind, body)) = next_record(&bytes[at..]) else {
            replayed.damaged = true;
            break;
        };
        let end = at + 9 + body.len();
        // The CRC matched, so this is no torn write: a record that still won't decode was written wrong, and
        // cutting it off would lose the good records after it.
        let applied = match kind {
            SESSION => match session_string::decode_fields(body) {
                Ok(s) => {
                    replayed.tables.session = Some(s);
                    true
                }
                Err(why) if why == session_string::NEWER_VAEGRAM => return refuse(NEWER),
                Err(_) => false,
            },
            STATE => match decode_state(body) {
                Some(s) => {
                    replayed.tables.state = Some(s);
                    true
                }
                None => false,
            },
            PEER => match decode_peer(body) {
                Some(p) => {
                    replayed.tables.upsert(p);
                    true
                }
                None => false,
            },
            CLEAR_PEERS => {
                replayed.tables.clear_peers();
                true
            }
            CRITICAL..=u8::MAX => return refuse(NEWER),
            _ => {
                replayed.skipped.extend_from_slice(&bytes[at..end]);
                true
            }
        };
        if !applied {
            return refuse(MALFORMED);
        }
        at = end;
        replayed.good_len = at;
    }
    Ok(replayed)
}

fn next_record(b: &[u8]) -> Option<(u8, &[u8])> {
    let (head, rest) = b.split_first_chunk::<8>()?;
    let len = u32::from_le_bytes(head[..4].try_into().unwrap()) as usize;
    let content = rest.get(..len).filter(|c| !c.is_empty())?;
    (crc32fast::hash(content).to_le_bytes() == head[4..]).then(|| (content[0], &content[1..]))
}

fn take<const N: usize>(b: &mut &[u8]) -> Option<[u8; N]> {
    let (head, rest) = b.split_first_chunk::<N>()?;
    *b = rest;
    Some(*head)
}

fn take_str(b: &mut &[u8]) -> Option<String> {
    let [len] = take::<1>(b)?;
    let (s, rest) = b.split_at_checked(len as usize)?;
    *b = rest;
    String::from_utf8(s.to_vec()).ok()
}

fn decode_state(mut b: &[u8]) -> Option<StoredState> {
    let owner = i64::from_le_bytes(take(&mut b)?);
    let salt = Salt { value: i64::from_le_bytes(take(&mut b)?), time_offset: i64::from_le_bytes(take(&mut b)?) };
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
    let update = (date != 0).then_some(UpdateState { pts, qts, date, seq, channels });
    b.is_empty().then_some(StoredState { owner, salt, update })
}

fn decode_peer(mut b: &[u8]) -> Option<PeerRecord> {
    let mut p = PeerRecord::new(i64::from_le_bytes(take(&mut b)?), i64::from_le_bytes(take(&mut b)?));
    p.updated = i64::from_le_bytes(take(&mut b)?);
    p.phone_number = Some(take_str(&mut b)?).filter(|phone| !phone.is_empty());
    let [count] = take::<1>(&mut b)?;
    p.usernames = (0..count).map(|_| take_str(&mut b)).collect::<Option<_>>()?;
    b.is_empty().then_some(p)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::storage::SessionData;

    fn sample() -> Tables {
        let mut tables = Tables::default();
        let mut session = SessionData::new(2, [7; 256]);
        session.user_id = 42;
        tables.session = Some(session);
        let update = UpdateState { pts: 5, qts: 1, date: 1_700_000_000, seq: 2, channels: HashMap::from([(9, 3)]) };
        tables.state = Some(StoredState::new(42, Salt { value: -7, time_offset: 3 }, Some(update)));
        let mut alice = PeerRecord::new(100, -5);
        (alice.usernames, alice.phone_number, alice.updated) = (vec!["alice".into()], Some("123".into()), 10);
        tables.upsert(alice);
        tables.upsert(PeerRecord::new(-1_000_000_000_009, 77));
        tables
    }

    fn same(a: &Tables, b: &Tables) -> bool {
        let session = |t: &Tables| t.session.as_ref().map(|s| (s.dc_id, s.auth_key, s.user_id));
        session(a) == session(b) && a.state == b.state && a.export(None) == b.export(None)
    }

    #[test]
    fn a_snapshot_replays_to_the_same_tables() {
        let bytes = snapshot(&sample(), &[]);
        assert_eq!(&bytes[..HEADER_LEN], &header());
        let replayed = replay(&bytes).unwrap();
        assert!(same(&replayed.tables, &sample()));
        assert_eq!((replayed.good_len, replayed.damaged), (bytes.len(), false));
    }

    #[test]
    fn later_records_win_and_clear_drops_earlier_peers() {
        let mut bytes = snapshot(&sample(), &[]);
        clear_peers(&mut bytes);
        let mut bob = PeerRecord::new(200, 1);
        bob.usernames = vec!["bob".into(), "robert".into()];
        peer(&mut bytes, &bob);
        state(&mut bytes, &StoredState::new(42, Salt::default(), None));
        let tables = replay(&bytes).unwrap().tables;
        assert_eq!(tables.export(None), std::slice::from_ref(&bob));
        assert_eq!(tables.by_username("robert"), Some(bob));
        assert_eq!(tables.state, Some(StoredState::new(42, Salt::default(), None)));
        assert_eq!(tables.session.map(|s| s.user_id), Some(42));
    }

    #[test]
    fn unknown_record_kinds_follow_the_tag_rules() {
        let mut skipped = snapshot(&sample(), &[]);
        let unknown_at = skipped.len();
        record(&mut skipped, 0x7f, |b| b.extend_from_slice(b"later"));
        let unknown = skipped[unknown_at..].to_vec();
        clear_peers(&mut skipped);
        let replayed = replay(&skipped).unwrap();
        assert_eq!((replayed.tables.export(None), replayed.damaged), (Vec::new(), false));
        assert_eq!(replayed.skipped, unknown, "a skipped record is kept whole");
        let compacted = snapshot(&replayed.tables, &replayed.skipped);
        assert_eq!(replay(&compacted).unwrap().skipped, unknown, "and survives compaction");

        for kind in [CRITICAL, 0xff] {
            let mut refused = snapshot(&sample(), &[]);
            record(&mut refused, kind, |_| {});
            assert_eq!(replay(&refused).err().map(|e| e.to_string()), Some(NEWER.to_owned()));
        }
        let mut newer_format = header();
        newer_format[16] = 2;
        assert_eq!(replay(&newer_format).err().map(|e| e.to_string()), Some(NEWER.to_owned()));
    }

    #[test]
    fn a_whole_record_that_wont_decode_refuses_the_file() {
        for (kind, body) in [(SESSION, &[1, 0x01, 1][..]), (STATE, &[0; 7][..]), (PEER, &[0; 25][..])] {
            let mut bytes = snapshot(&sample(), &[]);
            record(&mut bytes, kind, |b| b.extend_from_slice(body));
            peer(&mut bytes, &PeerRecord::new(5, 6));
            let error = replay(&bytes).err().unwrap();
            assert_eq!((error.kind(), error.to_string()), (io::ErrorKind::InvalidData, MALFORMED.to_owned()));
        }
    }

    #[test]
    fn username_owners_do_not_depend_on_record_order() {
        let mut old = PeerRecord::new(100, 1);
        (old.usernames, old.phone_number, old.updated) = (vec!["shared".into()], Some("555".into()), 10);
        let mut new = PeerRecord::new(200, 2);
        (new.usernames, new.phone_number, new.updated) = (vec!["shared".into()], Some("555".into()), 20);
        for order in [[&old, &new], [&new, &old]] {
            let mut tables = Tables::default();
            for p in order {
                tables.upsert(p.clone());
            }
            let compacted = replay(&snapshot(&tables, &[])).unwrap().tables;
            for t in [&tables, &compacted] {
                assert_eq!(t.by_username("shared").map(|p| p.id), Some(200));
                assert_eq!(t.by_phone_number("555").map(|p| p.id), Some(200));
            }
        }
        let mut tables = Tables::default();
        tables.upsert(old.clone());
        tables.upsert(new.clone());
        new.usernames.clear();
        tables.upsert(new);
        assert_eq!(tables.by_username("shared").map(|p| p.id), Some(100), "the other claim stays");
    }

    #[test]
    fn foreign_files_are_refused_and_partial_headers_are_new() {
        let message = |bytes: &[u8]| replay(bytes).err().map(|e| (e.kind(), e.to_string()));
        let mut sqlite = SQLITE_MAGIC.to_vec();
        sqlite.extend_from_slice(&[0; 84]);
        assert_eq!(message(&sqlite), Some((io::ErrorKind::InvalidData, SQLITE.to_owned())));
        for foreign in [&b"VAE_AQIAAA"[..], b"vaegram-session\0\0\0\0\0", &[0xff; 40]] {
            assert_eq!(message(foreign), Some((io::ErrorKind::InvalidData, FOREIGN.to_owned())));
        }
        for partial in [&[][..], &header()[..5], &header()[..19]] {
            let replayed = replay(partial).unwrap();
            assert_eq!((replayed.good_len, replayed.damaged), (0, !partial.is_empty()));
        }
    }

    #[test]
    fn short_and_damaged_records_end_the_replay() {
        let whole = snapshot(&sample(), &[]);
        let good = {
            let mut b = header().to_vec();
            session(&mut b, sample().session.as_ref().unwrap());
            b.len()
        };
        for cut in [good + 1, good + 8, good + 9, whole.len() - 1] {
            let replayed = replay(&whole[..cut]).unwrap();
            assert!(replayed.damaged && replayed.good_len >= good && replayed.good_len < cut, "{cut}");
        }
        let mut flipped = whole.clone();
        flipped[good + 12] ^= 1;
        let replayed = replay(&flipped).unwrap();
        assert_eq!((replayed.good_len, replayed.damaged), (good, true));
        assert!(replayed.tables.state.is_none() && replayed.tables.session.is_some());
    }

    #[test]
    fn long_usernames_and_phones_are_left_out() {
        let mut p = PeerRecord::new(1, 2);
        (p.usernames, p.phone_number) = (vec!["a".repeat(256), "ok".into()], Some("9".repeat(256)));
        let mut bytes = header().to_vec();
        peer(&mut bytes, &p);
        let back = replay(&bytes).unwrap().tables.by_id(1).unwrap();
        assert_eq!((back.usernames, back.phone_number), (vec!["ok".to_owned()], None));
    }

    #[test]
    fn replaying_garbage_never_panics() {
        let seed = {
            let mut b = snapshot(&sample(), &[]);
            clear_peers(&mut b);
            peer(&mut b, &PeerRecord::new(5, 6));
            b
        };
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = move |n: usize| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % n as u64) as usize
        };
        for _ in 0..20_000 {
            let mut b = seed.clone();
            for _ in 0..=next(4) {
                let at = next(b.len());
                match next(3) {
                    0 => b[at] = next(256) as u8,
                    1 => b.truncate(at + 1),
                    _ => b.insert(at, next(256) as u8),
                }
            }
            let _ = replay(&b);
            // The same damage inside a record with a valid CRC reaches the body decoders.
            if b.len() > HEADER_LEN + 9 {
                let mut resealed = header().to_vec();
                let kind = b[HEADER_LEN + 8];
                record(&mut resealed, kind, |out| out.extend_from_slice(&b[HEADER_LEN + 9..]));
                let _ = replay(&resealed);
            }
        }
    }
}
