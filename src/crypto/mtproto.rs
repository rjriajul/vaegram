pub use warpcrypto::{Error, kdf, unpack_message as unpack};

use std::ops::Range;

use sha2::{Digest, Sha256};
use warpcrypto::{ige256_decrypt, ige256_encrypt};

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

/// Bytes in front of the body: auth_key_id, msg_key, salt, session_id, msg_id, seq_no, length.
pub const HEADER: usize = 56;

pub fn header(packet: &mut [u8], salt: i64, session_id: &[u8; 8], msg_id: i64, seq_no: i32) {
    let len = (packet.len() - HEADER) as u32;
    let h = &mut packet[24..HEADER];
    h[..8].copy_from_slice(&salt.to_le_bytes());
    h[8..16].copy_from_slice(session_id);
    h[16..24].copy_from_slice(&msg_id.to_le_bytes());
    h[24..28].copy_from_slice(&seq_no.to_le_bytes());
    h[28..].copy_from_slice(&len.to_le_bytes());
}

pub fn seal(buf: &mut Vec<u8>, at: usize, auth_key: &[u8; 256], auth_key_id: &[u8; 8]) {
    let plain = buf.len() - at - 24;
    let end = buf.len();
    buf.resize(at + 24 + ((plain + 12 + 15) & !15), 0);
    random_fill(&mut buf[end..]);
    let digest = Sha256::new().chain_update(&auth_key[88..120]).chain_update(&buf[at + 24..]).finalize();
    let msg_key: [u8; 16] = digest[8..24].try_into().unwrap();
    let (key, iv) = kdf(auth_key, &msg_key, true);
    let _ = ige256_encrypt(&mut buf[at + 24..], &key, &iv);
    buf[at..at + 8].copy_from_slice(auth_key_id);
    buf[at + 8..at + 24].copy_from_slice(&msg_key);
}

/// Decrypts a server message in place and checks it; returns msg_id, seq_no and where the body is.
pub fn open(
    packet: &mut [u8],
    auth_key: &[u8; 256],
    auth_key_id: &[u8; 8],
    session_id: &[u8; 8],
) -> Result<(i64, i32, Range<usize>), Error> {
    if packet.len() < 24 {
        return Err(Error::TooShort);
    }
    if packet[..8] != *auth_key_id {
        return Err(Error::AuthKeyIdMismatch);
    }
    let (head, dec) = packet.split_at_mut(24);
    let msg_key: [u8; 16] = head[8..].try_into().unwrap();
    let (key, iv) = kdf(auth_key, &msg_key, false);
    ige256_decrypt(dec, &key, &iv)?;
    if dec.len() < 32 {
        return Err(Error::MsgKeyMismatch);
    }
    // https://core.telegram.org/mtproto/security_guidelines#checking-sha256-hash-value-of-msg-key
    let digest = Sha256::new().chain_update(&auth_key[96..128]).chain_update(&*dec).finalize();
    if digest[8..24] != msg_key {
        return Err(Error::MsgKeyMismatch);
    }
    if dec[8..16] != *session_id {
        return Err(Error::SessionIdMismatch);
    }
    let msg_id = i64::from_le_bytes(dec[16..24].try_into().unwrap());
    let seq_no = i32::from_le_bytes(dec[24..28].try_into().unwrap());
    let length = i32::from_le_bytes(dec[28..32].try_into().unwrap());
    let length = usize::try_from(length).map_err(|_| Error::NegativeLength)?;
    if 32 + length > dec.len() {
        return Err(Error::LengthExceedsData);
    }
    // https://core.telegram.org/mtproto/security_guidelines#checking-message-length
    if length % 4 != 0 {
        return Err(Error::LengthNotAligned);
    }
    if !(12..=1024).contains(&(dec.len() - 32 - length)) {
        return Err(Error::PaddingOutOfRange);
    }
    Ok((msg_id, seq_no, HEADER..HEADER + length))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 256] = {
        let mut k = [0; 256];
        let mut i = 0;
        while i < 256 {
            k[i] = i as u8;
            i += 1;
        }
        k
    };
    const KEY_ID: [u8; 8] = [1; 8];
    const SESSION: [u8; 8] = [2; 8];

    fn sealed(body: &[u8]) -> Vec<u8> {
        let mut buf = vec![9; 3 + HEADER];
        buf.extend_from_slice(body);
        header(&mut buf[3..], 77, &SESSION, 1 << 32, 5);
        seal(&mut buf, 3, &KEY, &KEY_ID);
        buf.drain(..3);
        buf
    }

    fn server_packet(body: &[u8], padding: usize) -> Vec<u8> {
        let mut plain = 0i64.to_le_bytes().to_vec();
        plain.extend_from_slice(&SESSION);
        plain.extend_from_slice(&((1i64 << 32) + 1).to_le_bytes());
        plain.extend_from_slice(&3i32.to_le_bytes());
        plain.extend_from_slice(&(body.len() as u32).to_le_bytes());
        plain.extend_from_slice(body);
        plain.resize(plain.len() + padding, 0xcc);
        let digest = Sha256::new().chain_update(&KEY[96..128]).chain_update(&plain).finalize();
        let msg_key: [u8; 16] = digest[8..24].try_into().unwrap();
        let (key, iv) = kdf(&KEY, &msg_key, false);
        ige256_encrypt(&mut plain, &key, &iv).unwrap();
        [&KEY_ID[..], &msg_key, &plain].concat()
    }

    #[test]
    fn seal_matches_warpcrypto() {
        for len in [0, 4, 200, 4096] {
            let body: Vec<u8> = (0..len).map(|i| i as u8).collect();
            let packet = sealed(&body);
            assert_eq!(packet.len() % 16, 8);
            let m = unpack(&packet, &KEY, &KEY_ID, &SESSION, false).unwrap();
            assert_eq!((m.msg_id, m.seq_no, m.body), (1 << 32, 5, body));
        }
    }

    #[test]
    fn opens_server_packets_in_place() {
        let body = [7u8; 40];
        let mut packet = server_packet(&body, 24);
        let (msg_id, seq_no, at) = open(&mut packet, &KEY, &KEY_ID, &SESSION).unwrap();
        assert_eq!((msg_id, seq_no, &packet[at]), ((1 << 32) + 1, 3, &body[..]));
    }

    #[test]
    fn rejects_what_unpack_rejects() {
        let good = server_packet(&[7; 40], 24);
        let mut flipped = good.clone();
        flipped[40] ^= 1;
        let mut bad_id = good.clone();
        bad_id[0] ^= 1;
        let cases = [
            (flipped, Error::MsgKeyMismatch),
            (bad_id, Error::AuthKeyIdMismatch),
            (good[..20].to_vec(), Error::TooShort),
            (server_packet(&[7; 40], 8), Error::PaddingOutOfRange),
            (server_packet(&[7; 42], 22), Error::LengthNotAligned),
        ];
        for (mut packet, err) in cases {
            assert_eq!(unpack(&packet, &KEY, &KEY_ID, &SESSION, true).unwrap_err(), err);
            assert_eq!(open(&mut packet, &KEY, &KEY_ID, &SESSION).unwrap_err(), err);
        }
        let mut other_session = good;
        assert_eq!(open(&mut other_session, &KEY, &KEY_ID, &[3; 8]).unwrap_err(), Error::SessionIdMismatch);
    }
}
