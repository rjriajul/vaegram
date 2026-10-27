// https://core.telegram.org/mtproto/mtproto-transports#transport-obfuscation

use crate::crypto::{aes, random_fill, sha256};

pub(crate) struct Ctr {
    key: [u8; 32],
    iv: [u8; 16],
    used: u8,
}

impl Ctr {
    fn new(key: &[u8], iv: &[u8], secret: Option<&[u8; 16]>) -> Self {
        let key = match secret {
            Some(s) => sha256(&[key, s]),
            None => key.try_into().unwrap(),
        };
        Ctr { key, iv: iv.try_into().unwrap(), used: 0 }
    }

    pub(crate) fn apply(&mut self, data: &mut [u8]) {
        aes::ctr256(data, &self.key, &mut self.iv, &mut self.used);
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
}
