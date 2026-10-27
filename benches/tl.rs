//! TL and session micro-benchmarks in plain std: `cargo bench --bench tl`.

#[path = "support/timing.rs"]
mod timing;

use std::hint::black_box;

use timing::bench;
use vaegram::crypto::mtproto;
use vaegram::raw::{Deserializable, Identifiable, Serializable, base, functions, types};
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
    let body = ping.to_bytes();
    bench("mtproto pack ping", || {
        black_box(mtproto::pack(&auth_key, &auth_key_id, &session_id, 0, 1 << 32, 1, black_box(&body)));
    });
    let packed = mtproto::pack(&auth_key, &auth_key_id, &session_id, 0, 1 << 32, 1, &body);
    bench("mtproto unpack ping", || {
        black_box(mtproto::unpack(black_box(&packed), &auth_key, &auth_key_id, &session_id, false).unwrap());
    });
    let packed = mtproto::pack(&auth_key, &auth_key_id, &session_id, 0, 1 << 32, 1, &part_bytes);
    bench("mtproto pack 512K part", || {
        black_box(mtproto::pack(&auth_key, &auth_key_id, &session_id, 0, 1 << 32, 1, black_box(&part_bytes)));
    });
    bench("mtproto unpack 512K part", || {
        black_box(mtproto::unpack(black_box(&packed), &auth_key, &auth_key_id, &session_id, false).unwrap());
    });

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
