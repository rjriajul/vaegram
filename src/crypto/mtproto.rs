pub use warpcrypto::{kdf, pack_message as pack, unpack_message as unpack};

use super::{aes, random, random_fill, sha1};

pub(crate) fn auth_key_id(key: &[u8; 256]) -> [u8; 8] {
    sha1(&[key])[12..].try_into().unwrap()
}

pub(crate) fn kdf_v1(key: &[u8; 256], msg_key: &[u8; 16]) -> ([u8; 32], [u8; 32]) {
    let a = sha1(&[msg_key, &key[..32]]);
    let b = sha1(&[&key[32..48], msg_key, &key[48..64]]);
    let c = sha1(&[&key[64..96], msg_key]);
    let d = sha1(&[msg_key, &key[96..128]]);
    let mut aes_key = [0; 32];
    aes_key[..8].copy_from_slice(&a[..8]);
    aes_key[8..20].copy_from_slice(&b[8..]);
    aes_key[20..].copy_from_slice(&c[4..16]);
    let mut iv = [0; 32];
    iv[..12].copy_from_slice(&a[8..]);
    iv[12..20].copy_from_slice(&b[..8]);
    iv[20..24].copy_from_slice(&c[16..]);
    iv[24..].copy_from_slice(&d[..8]);
    (aes_key, iv)
}

/// `body` sealed with MTProto v1: a random int128 stands in for salt and session_id, seq_no is 0.
pub(crate) fn encrypt_v1(key: &[u8; 256], msg_id: i64, body: &[u8]) -> Vec<u8> {
    let len = (body.len() as u32).to_le_bytes();
    let mut plain = [&random::<16>()[..], &msg_id.to_le_bytes(), &0i32.to_le_bytes(), &len, body].concat();
    let msg_key: [u8; 16] = sha1(&[&plain])[4..].try_into().unwrap();
    let len = plain.len();
    plain.resize(len.next_multiple_of(16), 0);
    random_fill(&mut plain[len..]);
    let (aes_key, iv) = kdf_v1(key, &msg_key);
    aes::ige256_encrypt(&mut plain, &aes_key, &iv).unwrap();

    [&auth_key_id(key)[..], &msg_key, &plain].concat()
}
