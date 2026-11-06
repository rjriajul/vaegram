// https://core.telegram.org/mtproto/mtproto-transports#transport-obfuscation

use warpcrypto::Ctr256;

use crate::crypto::{random_fill, sha256};

/// One direction of the obfuscated stream. The key is expanded once, not on every packet, and its schedule is wiped on
/// drop.
pub(crate) struct Ctr(Ctr256);

impl Ctr {
    fn new(key: &[u8], iv: &[u8], secret: Option<&[u8; 16]>) -> Self {
        let key = match secret {
            Some(s) => sha256(&[key, s]),
            None => key.try_into().unwrap(),
        };
        Ctr(Ctr256::new(&key, iv.try_into().unwrap()))
    }

    pub(crate) fn apply(&mut self, data: &mut [u8]) {
        self.0.apply_keystream(data);
    }
}

const FORBIDDEN: [[u8; 4]; 7] =
    [*b"HEAD", *b"POST", *b"GET ", *b"OPTI", [0x16, 0x03, 0x01, 0x02], [0xdd; 4], [0xee; 4]];

pub(crate) fn init(tag: [u8; 4], dc_id: i16, secret: Option<&[u8; 16]>) -> ([u8; 64], Ctr, Ctr) {
    let mut header = [0u8; 64];
    loop {
        random_fill(&mut header);
        if header[0] != 0xef && !FORBIDDEN.iter().any(|f| header[..4] == *f) && header[4..8] != [0; 4] {
            break;
        }
    }
    header[56..60].copy_from_slice(&tag);
    header[60..62].copy_from_slice(&dc_id.to_le_bytes());

    let mut reversed = [0u8; 48];
    reversed.copy_from_slice(&header[8..56]);
    reversed.reverse();
    let mut encrypt = Ctr::new(&header[8..40], &header[40..56], secret);
    let decrypt = Ctr::new(&reversed[..32], &reversed[32..], secret);

    let mut encrypted = header;
    encrypt.apply(&mut encrypted);
    header[56..].copy_from_slice(&encrypted[56..]);
    (header, encrypt, decrypt)
}

#[cfg(test)]
mod tests {
    use super::{Ctr, FORBIDDEN, init};
    use crate::crypto::aes;

    // The server decrypts with our encrypt key and encrypts with our decrypt key.
    fn server(header: &[u8; 64], secret: Option<&[u8; 16]>) -> ([u8; 64], Ctr, Ctr) {
        let mut reversed = [0u8; 48];
        reversed.copy_from_slice(&header[8..56]);
        reversed.reverse();
        let mut decrypt = Ctr::new(&header[8..40], &header[40..56], secret);
        let encrypt = Ctr::new(&reversed[..32], &reversed[32..], secret);
        let mut plain = *header;
        decrypt.apply(&mut plain);
        (plain, decrypt, encrypt)
    }

    #[test]
    fn header_avoids_forbidden_prefixes() {
        for _ in 0..2000 {
            let (h, _, _) = init([0xef; 4], 2, None);
            assert_ne!(h[0], 0xef);
            assert!(!FORBIDDEN.contains(&h[..4].try_into().unwrap()));
            assert_ne!(h[4..8], [0; 4]);
        }
    }

    #[test]
    fn server_reads_tag_and_dc_and_both_directions_round_trip() {
        for secret in [None, Some(&[7u8; 16])] {
            let (header, mut encrypt, mut decrypt) = init([0xdd; 4], -4, secret);
            let (plain, mut server_decrypt, mut server_encrypt) = server(&header, secret);
            assert_eq!(plain[56..60], [0xdd; 4]);
            assert_eq!(i16::from_le_bytes([plain[60], plain[61]]), -4);

            let message: Vec<u8> = (0..300u16).map(|i| i as u8).collect();
            let mut wire = message.clone();
            for chunk in wire.chunks_mut(7) {
                encrypt.apply(chunk);
            }
            assert_ne!(wire, message);
            for chunk in wire.chunks_mut(13) {
                server_decrypt.apply(chunk);
            }
            assert_eq!(wire, message);

            server_encrypt.apply(&mut wire);
            for chunk in wire.chunks_mut(1) {
                decrypt.apply(chunk);
            }
            assert_eq!(wire, message);
        }
    }

    #[test]
    fn secret_changes_keys() {
        let (header, mut encrypt, _) = init([0xee; 4], 2, Some(&[1; 16]));
        let (_, mut plain_decrypt, _) = server(&header, None);
        let mut data = [0u8; 32];
        encrypt.apply(&mut data);
        plain_decrypt.apply(&mut data);
        assert_ne!(data, [0; 32]);
    }

    #[test]
    fn matches_one_shot_ctr_across_chunk_sizes() {
        let (key, iv) = ([9u8; 32], [0xff; 16]);
        let data: Vec<u8> = (0..1000u16).map(|i| (i * 7) as u8).collect();
        let mut expected = data.clone();
        aes::ctr256(&mut expected, &key, &mut iv.clone(), &mut 0);
        for size in [1, 15, 16, 17, 129, 200, 1000] {
            let mut ctr = Ctr::new(&key, &iv, None);
            let mut got = data.clone();
            for chunk in got.chunks_mut(size) {
                ctr.apply(chunk);
            }
            assert_eq!(got, expected, "chunk size {size}");
        }
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    // NIST SP 800-38A, F.5.5 CTR-AES256.Encrypt.
    #[test]
    fn nist_ctr_aes256_known_answer() {
        let key = hex("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4");
        let iv = hex("f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff");
        let mut data = hex(concat!(
            "6bc1bee22e409f96e93d7e117393172a",
            "ae2d8a571e03ac9c9eb76fac45af8e51",
            "30c81c46a35ce411e5fbc1191a0a52ef",
            "f69f2445df4f9b17ad2b417be66c3710",
        ));
        let expected = hex(concat!(
            "601ec313775789a5b7a7f504bbf3d228",
            "f443e3ca4d62b59aca84e990cacaf5c5",
            "2b0930daa23de94ce87017ba2d84988d",
            "dfc9c58db67aada613c2dd08457941a6",
        ));
        let mut ctr = Ctr::new(&key, &iv, None);
        for chunk in data.chunks_mut(7) {
            ctr.apply(chunk);
        }
        assert_eq!(data, expected);
    }
}
