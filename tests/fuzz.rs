use std::collections::HashMap;
use std::io::Write;

use vaegram::connection::Transport;
use vaegram::raw::core::{gzip_packed, msg_container};
use vaegram::raw::{Deserializable, Serializable, base, types};
use vaegram::session::Salt;
use vaegram::storage::state::{self, Peers, UpdateState};
use vaegram::storage::{SessionData, session_string};

const MAX_UNPACKED: usize = 32 << 20;
const VECTOR: u32 = 0x1cb5c415;
const RPC_RESULT: u32 = 0xf35c6d01;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.next() as u8).collect()
    }

    // Small edits keep most of the structure, so decoding gets past the first constructor. Copied words are
    // often constructor ids or lengths, which reach deeper than random ones.
    fn mutate(&mut self, seed: &[u8]) -> Vec<u8> {
        let mut b = seed.to_vec();
        for _ in 0..1 + self.below(4) {
            let i = self.below(b.len());
            match self.below(6) {
                0 if i < b.len() => b[i] ^= 1 << self.below(8),
                1 if i < b.len() => b[i] = [0, 1, 0x7f, 0x80, 0xfe, 0xff][self.below(6)],
                2 if i + 4 <= b.len() => {
                    let j = self.below(b.len() - 3);
                    let word = match self.below(3) {
                        0 => [0xff; 4],
                        1 => i32::MAX.to_le_bytes(),
                        _ => b[j..j + 4].try_into().unwrap(),
                    };
                    b[i..i + 4].copy_from_slice(&word);
                }
                3 => b.truncate(i),
                4 => {
                    let n = self.below(16);
                    let noise = self.bytes(n);
                    b.splice(i..i, noise);
                }
                _ => {
                    let end = i + self.below(b.len() - i + 1);
                    let chunk = b[i..end].to_vec();
                    let at = self.below(b.len() + 1);
                    b.splice(at..at, chunk);
                }
            }
        }
        b
    }
}

fn tl(parts: &[&dyn Serializable]) -> Vec<u8> {
    let mut b = Vec::new();
    parts.iter().for_each(|p| p.serialize(&mut b));
    b
}

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(data).unwrap();
    tl(&[&gzip_packed::ID, &gz.finish().unwrap()])
}

// message: out, media (photo, empty), two entities, then peerUser, date and text.
fn message() -> Vec<u8> {
    let flags = 1u32 << 1 | 1 << 7 | 1 << 9;
    let (text, url) = ("hello *bold*".to_string(), "https://t.me".to_string());
    let (bold, text_url) = (0xbd610bc9u32, 0x76a6d327u32);
    let head = tl(&[&0x7600b9d3u32, &flags, &0u32, &42, &0x59511722u32, &777000i64, &1_700_000_000, &text]);
    let media_and_entities = tl(&[&0xe216eb63u32, &0u32, &VECTOR, &2u32, &bold, &6, &6, &text_url, &0, &5, &url]);
    [head, media_and_entities].concat()
}

fn updates() -> Vec<u8> {
    let new_message = [tl(&[&0x74ae4240u32, &VECTOR, &2u32, &0x1f2b0afdu32]), message(), tl(&[&40, &1])].concat();
    let deleted = tl(&[&0xa20db0e5u32, &VECTOR, &2u32, &1, &2, &42, &2]);
    let rest = tl(&[&VECTOR, &1u32, &0xd3bc4b7au32, &777000i64, &VECTOR, &0u32, &1_700_000_000, &5]);
    [new_message, deleted, rest].concat()
}

fn container(messages: &[Vec<u8>]) -> Vec<u8> {
    let mut b = tl(&[&msg_container::ID, &(messages.len() as u32)]);
    for (i, m) in messages.iter().enumerate() {
        b.extend(tl(&[&(i as i64 * 4 + 1), &(i as i32 * 2 + 1), &(m.len() as u32)]));
        b.extend(m);
    }
    b
}

fn corpus() -> Vec<Vec<u8>> {
    let updates = updates();
    let rpc_result = |result: &[u8]| [tl(&[&RPC_RESULT, &7i64]), result.to_vec()].concat();
    let flood = tl(&[&0x2144ca19u32, &420, &"FLOOD_WAIT_30".to_string()]);
    let short = tl(&[&0x78d4dec1u32, &0xa20db0e5u32, &VECTOR, &1u32, &5, &10, &1, &1_700_000_000]);
    let mut all =
        vec![message(), short, rpc_result(&updates), rpc_result(&flood), rpc_result(&gzip(&updates)), gzip(&updates)];
    all.push(container(&all));
    all.push(updates);
    all
}

fn constructor_id(b: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(..4)?.try_into().unwrap()))
}

fn packet(body: &[u8]) {
    if constructor_id(body) == Some(msg_container::ID) {
        for m in msg_container::parse(body).unwrap_or_default() {
            message_body(m.body);
        }
    } else {
        message_body(body);
    }
}

fn message_body(body: &[u8]) {
    match constructor_id(body) {
        Some(gzip_packed::ID) => {
            if let Ok(b) = gzip_packed::unpack(body) {
                assert!(b.len() <= MAX_UNPACKED);
                message_body(&b);
            }
        }
        Some(RPC_RESULT) => message_body(body.get(12..).unwrap_or_default()),
        _ => {
            let _ = base::Updates::from_bytes(body);
            let _ = base::Update::from_bytes(body);
            let _ = base::Message::from_bytes(body);
            let _ = base::messages::Messages::from_bytes(body);
            let _ = base::RpcError::from_bytes(body);
        }
    }
}

#[test]
fn mutated_tl_never_panics() {
    let corpus = corpus();
    for c in &corpus {
        packet(c);
    }
    assert!(base::Message::from_bytes(&corpus[0]).is_ok());
    assert!(base::Updates::from_bytes(&corpus[corpus.len() - 1]).is_ok());

    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for _ in 0..20_000 {
        let seed = &corpus[rng.below(corpus.len())];
        packet(&rng.mutate(seed));
    }
    for len in 0..512 {
        let mut b = rng.bytes(len);
        let seed = &corpus[rng.below(corpus.len())];
        let n = len.min(4);
        b[..n].copy_from_slice(&seed[..n]);
        packet(&b);
    }
}

// A fixed-Huffman deflate stream of one zero and then 258-byte copies, 13 bits each: ~1 KiB per 20 KiB unpacked.
fn gzip_bomb(unpacked: usize) -> Vec<u8> {
    let mut gz = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 0xff];
    let (mut acc, mut bits) = (0u32, 0);
    let mut put = |gz: &mut Vec<u8>, code: u32, n: u32| {
        for i in (0..n).rev() {
            acc |= (code >> i & 1) << bits;
            bits += 1;
        }
        while bits >= 8 {
            gz.push(acc as u8);
            acc >>= 8;
            bits -= 8;
        }
    };
    put(&mut gz, 0b110, 3);
    put(&mut gz, 0x30, 8);
    for _ in 0..unpacked / 258 {
        put(&mut gz, 0xc5, 8);
        put(&mut gz, 0, 5);
    }
    put(&mut gz, 0, 7 + 7);
    gz.extend([0; 8]);
    tl(&[&gzip_packed::ID, &gz])
}

#[test]
fn gzip_bombs_stop_at_the_cap() {
    assert_eq!(gzip_packed::unpack(&gzip_bomb(MAX_UNPACKED + (1 << 20))).map(|b| b.len()), Ok(MAX_UNPACKED));
}

#[test]
fn deep_nesting_is_an_error_not_a_stack_overflow() {
    let bold = |depth: usize| [tl(&[&0x6724abc4u32]).repeat(depth), tl(&[&0xdc3d824fu32])].concat();
    assert!(base::RichText::from_bytes(&bold(20)).is_ok());
    assert!(base::RichText::from_bytes(&bold(100_000)).is_err());

    let array = tl(&[&0xf7444763u32, &VECTOR, &1u32]).repeat(100_000);
    assert!(base::JSONValue::from_bytes(&[array, tl(&[&0x3f6d7b68u32])].concat()).is_err());
}

#[tokio::test]
async fn transports_survive_garbage() {
    let encrypted: Vec<u8> = (0..24 + 32).map(|i| i as u8 | 1).collect();
    let plain = [vec![0; 16], 8u32.to_le_bytes().to_vec(), b"abcd1234".to_vec()].concat();
    let mut rng = Rng(7);
    for transport in [Transport::Abridged, Transport::Intermediate, Transport::PaddedIntermediate] {
        let mut frames = Vec::new();
        for payload in [&encrypted[..], &plain, &(-404i32).to_le_bytes()] {
            let mut frame = Vec::new();
            transport.encode(payload, &mut frame);
            frames.push(frame);
        }
        for i in 0..5000 {
            let input = match i % 3 {
                2 => {
                    let n = rng.below(64);
                    rng.bytes(n)
                }
                _ => {
                    let frame = &frames[rng.below(frames.len())];
                    rng.mutate(frame)
                }
            };
            if let Ok(p) = transport.read(&mut input.as_slice()).await {
                assert!(p.len() <= input.len(), "{transport:?} {input:?}");
            }
        }
        for header in [[0xff; 4], [0x7f, 0xff, 0xff, 0xff], (16u32 << 20 | 4).to_le_bytes()] {
            assert!(transport.read(&mut &header[..]).await.is_err(), "{transport:?} {header:?}");
        }
    }
}

#[test]
fn session_strings_survive_garbage() {
    let mut s = SessionData {
        dc_id: 2,
        test_mode: false,
        addr: Some("149.154.167.51:443".parse().unwrap()),
        api_id: 12345,
        auth_key: [7; 256],
        user_id: 777000,
        is_bot: false,
    };
    let v4 = session_string::export(&s);
    s.addr = None;
    let seeds = [v4, session_string::export(&s)];

    let mut rng = Rng(3);
    for _ in 0..10_000 {
        let seed = seeds[rng.below(2)].as_bytes();
        let b = rng.mutate(seed);
        let _ = session_string::parse(&String::from_utf8_lossy(&b));
    }
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    for len in (0..450).chain([362, 392, 435]) {
        let s: String = (0..len).map(|_| ALPHABET[rng.below(64)] as char).collect();
        let _ = session_string::parse(&s);
        let _ = session_string::parse(&format!("{}{s}", session_string::PREFIX));
    }
}

#[test]
fn state_files_survive_garbage() {
    let channel = types::InputPeerChannel { channel_id: 1234, access_hash: 9 };
    let peers = Peers::from([(-1_000_000_001_234, base::InputPeer::InputPeerChannel(Box::new(channel)))]);
    let update_state = UpdateState { pts: 10, qts: 2, date: 1, seq: 5, channels: HashMap::from([(1234, 99)]) };
    let bytes = state::encode(1, Salt { value: 7, time_offset: -3 }, &peers, Some(&update_state));

    let mut rng = Rng(11);
    for _ in 0..10_000 {
        let _ = state::decode(&rng.mutate(&bytes), 1);
    }

    let mut min_id = state::encode(1, Salt::default(), &Peers::new(), None);
    let n = min_id.len();
    min_id[n - 4..].copy_from_slice(&1u32.to_le_bytes());
    min_id.extend(i64::MIN.to_le_bytes());
    min_id.extend(0i64.to_le_bytes());
    let _ = state::decode(&min_id, 1);
}

#[test]
fn extreme_peer_ids_do_not_overflow() {
    use vaegram::utils::{get_peer, get_peer_id};
    for id in [i64::MIN, i64::MAX] {
        get_peer_id(&base::Peer::PeerChat(types::PeerChat { chat_id: id }));
        get_peer_id(&base::Peer::PeerChannel(types::PeerChannel { channel_id: id }));
        get_peer(id);
    }
}
