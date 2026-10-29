//! TL and session micro-benchmarks in plain std: `cargo bench --bench tl`.

#[path = "support/seal.rs"]
mod seal;
#[path = "support/timing.rs"]
mod timing;

use std::hint::black_box;
use std::io::{Read, Write};

use timing::bench;
use vaegram::connection::{Connection, Options};
use vaegram::crypto::mtproto;
use vaegram::raw::{Deserializable, Identifiable, Serializable, base, functions, types};
use vaegram::session::internals::data_center::Dc;
use vaegram::storage::{SessionData, session_string};

const VECTOR: u32 = 0x1cb5c415;
const TEXT: &str = "Hello there, this is a fairly ordinary chat message.";

fn message_bytes(id: i32, out: &mut Vec<u8>) {
    types::Message::CONSTRUCTOR_ID.serialize(out);
    0u32.serialize(out);
    0u32.serialize(out);
    id.serialize(out);
    types::PeerUser::CONSTRUCTOR_ID.serialize(out);
    777000i64.serialize(out);
    1_700_000_000i32.serialize(out);
    TEXT.to_owned().serialize(out);
}

fn updates_bytes(n: usize) -> Vec<u8> {
    let mut b = Vec::new();
    types::Updates::CONSTRUCTOR_ID.serialize(&mut b);
    VECTOR.serialize(&mut b);
    (n as u32).serialize(&mut b);
    for i in 1..=n as i32 {
        types::UpdateNewMessage::CONSTRUCTOR_ID.serialize(&mut b);
        message_bytes(i, &mut b);
        i.serialize(&mut b);
        1i32.serialize(&mut b);
    }
    for _ in 0..2 {
        VECTOR.serialize(&mut b);
        0u32.serialize(&mut b);
    }
    1_700_000_000i32.serialize(&mut b);
    0i32.serialize(&mut b);
    b
}

fn peer_user(b: &mut Vec<u8>, user_id: i64) {
    types::PeerUser::CONSTRUCTOR_ID.serialize(b);
    user_id.serialize(b);
}

fn new_message_updates() -> Vec<u8> {
    let mut b = Vec::new();
    types::Updates::CONSTRUCTOR_ID.serialize(&mut b);

    VECTOR.serialize(&mut b);
    1i32.serialize(&mut b);
    types::UpdateNewMessage::CONSTRUCTOR_ID.serialize(&mut b);
    types::Message::CONSTRUCTOR_ID.serialize(&mut b);
    ((1u32 << 8) | (1 << 7)).serialize(&mut b); // from_id, entities
    0u32.serialize(&mut b);
    42i32.serialize(&mut b);
    peer_user(&mut b, 777000);
    peer_user(&mut b, 777000);
    1_700_000_000i32.serialize(&mut b);
    "hello there, see https://example.com".to_owned().serialize(&mut b);
    VECTOR.serialize(&mut b);
    2i32.serialize(&mut b);
    types::MessageEntityBold::CONSTRUCTOR_ID.serialize(&mut b);
    0i32.serialize(&mut b);
    5i32.serialize(&mut b);
    types::MessageEntityTextUrl::CONSTRUCTOR_ID.serialize(&mut b);
    17i32.serialize(&mut b);
    19i32.serialize(&mut b);
    "https://example.com".to_owned().serialize(&mut b);
    1000i32.serialize(&mut b);
    1i32.serialize(&mut b);

    VECTOR.serialize(&mut b);
    1i32.serialize(&mut b);
    types::User::CONSTRUCTOR_ID.serialize(&mut b);
    (1u32 | (1 << 1) | (1 << 3) | (1 << 5) | (1 << 6) | (1 << 22)).serialize(&mut b);
    (1u32 << 8).serialize(&mut b); // color
    777000i64.serialize(&mut b);
    0x1234_5678_9abc_def0i64.serialize(&mut b);
    "Alice".to_owned().serialize(&mut b);
    "alice".to_owned().serialize(&mut b);
    types::UserProfilePhoto::CONSTRUCTOR_ID.serialize(&mut b);
    (1u32 << 1).serialize(&mut b);
    5_000_000_000i64.serialize(&mut b);
    vec![1u8; 33].serialize(&mut b);
    2i32.serialize(&mut b);
    types::UserStatusOnline::CONSTRUCTOR_ID.serialize(&mut b);
    1_700_000_300i32.serialize(&mut b);
    "en".to_owned().serialize(&mut b);
    types::PeerColor::CONSTRUCTOR_ID.serialize(&mut b);
    1u32.serialize(&mut b);
    3i32.serialize(&mut b);

    VECTOR.serialize(&mut b);
    0i32.serialize(&mut b);
    1_700_000_000i32.serialize(&mut b);
    0i32.serialize(&mut b);
    b
}

fn main() {
    timing::header();

    let ping = functions::Ping { ping_id: 0x1234_5678 };
    let mut buf = Vec::with_capacity(1 << 20);
    bench("ping serialize (reused buf)", || {
        buf.clear();
        black_box(&ping).serialize(&mut buf);
        black_box(&buf);
    });
    bench("ping to_bytes", || {
        black_box(black_box(&ping).to_bytes());
    });
    let pong = types::Pong { msg_id: 1, ping_id: 0x1234_5678 }.to_bytes();
    bench("pong deserialize", || {
        black_box(base::Pong::from_bytes(black_box(&pong)).unwrap());
    });

    let mut msg = Vec::new();
    message_bytes(42, &mut msg);
    bench("message deserialize", || {
        black_box(base::Message::from_bytes(black_box(&msg)).unwrap());
    });
    let parsed = base::Message::from_bytes(&msg).unwrap();
    assert_eq!(parsed.to_bytes(), msg);
    bench("message serialize", || {
        buf.clear();
        black_box(&parsed).serialize(&mut buf);
        black_box(&buf);
    });

    let typical = new_message_updates();
    let parsed = base::Updates::from_bytes(&typical).unwrap();
    assert_eq!(parsed.to_bytes(), typical);
    bench("typical new-message updates deserialize", || {
        black_box(base::Updates::from_bytes(black_box(&typical)).unwrap());
    });
    bench("typical new-message updates serialize", || {
        buf.clear();
        black_box(&parsed).serialize(&mut buf);
        black_box(&buf);
    });

    for n in [1, 10, 100] {
        let raw = updates_bytes(n);
        bench(&format!("updates x{n} deserialize"), || {
            black_box(base::Updates::from_bytes(black_box(&raw)).unwrap());
        });
        let parsed = base::Updates::from_bytes(&raw).unwrap();
        assert_eq!(parsed.to_bytes(), raw);
        bench(&format!("updates x{n} serialize"), || {
            buf.clear();
            black_box(&parsed).serialize(&mut buf);
            black_box(&buf);
        });
    }

    let part = functions::upload::SaveFilePart { file_id: 1, file_part: 0, bytes: vec![0xab; 512 * 1024] };
    bench("save_file_part 512K ser", || {
        buf.clear();
        black_box(&part).serialize(&mut buf);
        black_box(&buf);
    });
    let part_bytes = part.to_bytes();
    bench("bytes 512K deserialize", || {
        black_box(Vec::<u8>::from_bytes(black_box(&part_bytes[16..])).unwrap());
    });

    let auth_key = [7u8; 256];
    let auth_key_id = [1u8; 8];
    let session_id = [2u8; 8];
    let mut packet = Vec::new();
    for (name, body) in [("ping", ping.to_bytes()), ("512K part", part_bytes)] {
        bench(&format!("mtproto seal {name}"), || {
            packet.clear();
            packet.resize(mtproto::HEADER, 0);
            packet.extend_from_slice(black_box(&body));
            mtproto::header(&mut packet, 0, &session_id, 1 << 32, 1);
            mtproto::seal(&mut packet, 0, &auth_key, &auth_key_id);
        });
        // Each round copies the ciphertext back in, since opening decrypts it in place.
        let sealed = seal::seal(&auth_key, &auth_key_id, &session_id, (1 << 32) + 1, 1, &body);
        bench(&format!("mtproto open {name}"), || {
            packet.clear();
            packet.extend_from_slice(black_box(&sealed));
            black_box(mtproto::open(&mut packet, &auth_key, &auth_key_id, &session_id).unwrap());
        });
    }

    for size in [256, 64 << 10] {
        recv_loopback(size);
    }

    let session =
        SessionData { dc_id: 2, test_mode: false, addr: None, api_id: 12345, auth_key, user_id: 777000, is_bot: true };
    bench("session string export", || {
        black_box(session_string::export(black_box(&session)));
    });
    let exported = session_string::export(&session);
    bench("session string parse", || {
        black_box(session_string::parse(black_box(&exported)).unwrap());
    });
}

// A loopback server streams `size`-byte frames; each op reads one through the real Connection,
// so syscalls and buffering are part of the number.
fn recv_loopback(size: usize) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let mut frames = Vec::new();
    while frames.len() < 1 << 20 {
        frames.extend_from_slice(&(size as u32).to_le_bytes());
        frames.resize(frames.len() + size, 0x5a);
    }
    std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        s.read_exact(&mut [0; 4]).unwrap();
        while s.write_all(&frames).is_ok() {}
    });
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let dc = Dc { id: 2, addr, test: false };
    let mut conn = rt.block_on(Connection::connect(&dc, &Options::default())).unwrap();
    bench(&format!("recv loopback {size} B"), || {
        black_box(rt.block_on(conn.recv()).unwrap());
    });
}
