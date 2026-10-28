use num_bigint::BigUint;

use super::{aes, random, random_fill, sha1, sha256, to_be_padded};
use crate::raw::Serializable;

/// Telegram's production server key (`-----BEGIN RSA PUBLIC KEY----- MIIBCgKCAQEA6LszBcC1LGzyr992...`),
/// fingerprint 0xd09d1d85de64fd85. Exponent 65537.
const PRODUCTION_N_HEX: &str = concat!(
    "E8BB3305C0B52C6CF2AFDF7637313489E63E05268E5BADB601AF417786472E5F",
    "93B85438968E20E6729A301C0AFC121BF7151F834436F7FDA680847A66BF64AC",
    "CEC78EE21C0B316F0EDAFE2F41908DA7BD1F4A5107638EEB67040ACE472A14F9",
    "0D9F7C2B7DEF99688BA3073ADB5750BB02964902A359FE745D8170E36876D4FD",
    "8A5D41B2A76CBFF9A13267EB9580B2D06D10357448D20D9DA2191CB5D8C93982",
    "961CDFDEDA629E37F1FB09A0722027696032FE61ED663DB7A37F6F263D370F69",
    "DB53A0DC0A1748BDAAFF6209D5645485E6E001D1953255757E4B8E42813347B1",
    "1DA6AB500FD0ACE7E6DFA3736199CCAF9397ED0745A427DCFA6CD67BCB1ACFF3",
);

/// Test servers' key (PEM body `MIIBCgKCAQEAyMEdY1aR+sCR3ZSJrtztKTKqigvO...`), fingerprint
/// 0xb25898df208d2603. Test DCs don't offer the production key.
const TEST_N_HEX: &str = concat!(
    "C8C11D635691FAC091DD9489AEDCED2932AA8A0BCEFEF05FA800892D9B52ED03",
    "200865C9E97211CB2EE6C7AE96D3FB0E15AEFFD66019B44A08A240CFDD2868A8",
    "5E1F54D6FA5DEAA041F6941DDF302690D61DC476385C2FA655142353CB4E4B59",
    "F6E5B6584DB76FE8B1370263246C010C93D011014113EBDF987D093F9D37C2BE",
    "48352D69A1683F8F6E6C2167983C761E3AB169FDE5DAAA12123FA1BEAB621E4D",
    "A5935E9C198F82F35EAE583A99386D8110EA6BD1ABB0F568759F62694419EA5F",
    "69847C43462ABEF858B4CB5EDC84E7B9226CD7BD7E183AA974A712C079DDE85B",
    "9DC063B8A5C08E8F859C0EE5DCD824C7807F20153361A7F63CFD2A433A1BE7F5",
);

pub struct PublicKey {
    n: BigUint,
    e: BigUint,
}

impl PublicKey {
    pub fn production() -> Self {
        Self { n: BigUint::parse_bytes(PRODUCTION_N_HEX.as_bytes(), 16).unwrap(), e: BigUint::from(65537u32) }
    }

    pub fn test() -> Self {
        Self { n: BigUint::parse_bytes(TEST_N_HEX.as_bytes(), 16).unwrap(), e: BigUint::from(65537u32) }
    }

    pub fn fingerprint(&self) -> i64 {
        let mut buf = Vec::new();
        self.n.to_bytes_be().serialize(&mut buf);
        self.e.to_bytes_be().serialize(&mut buf);
        i64::from_le_bytes(sha1(&[&buf])[12..].try_into().unwrap())
    }

    pub fn encrypt_pad(&self, data: &[u8]) -> [u8; 256] {
        assert!(data.len() <= 144, "RSA_PAD data is {} bytes; at most 144 fit", data.len());
        let mut padded = [0u8; 192];
        padded[..data.len()].copy_from_slice(data);
        random_fill(&mut padded[data.len()..]);
        let mut reversed = padded;
        reversed.reverse();

        loop {
            let temp_key: [u8; 32] = random();
            let mut aes_encrypted = [0u8; 224];
            aes_encrypted[..192].copy_from_slice(&reversed);
            aes_encrypted[192..].copy_from_slice(&sha256(&[&temp_key, &padded]));
            aes::ige256_encrypt(&mut aes_encrypted, &temp_key, &[0; 32]).unwrap();

            let hash = sha256(&[&aes_encrypted]);
            let mut key_aes_encrypted = [0u8; 256];
            for i in 0..32 {
                key_aes_encrypted[i] = temp_key[i] ^ hash[i];
            }
            key_aes_encrypted[32..].copy_from_slice(&aes_encrypted);

            let x = BigUint::from_bytes_be(&key_aes_encrypted);
            if x < self.n {
                return to_be_padded(&x.modpow(&self.e, &self.n));
            }
        }
    }
}
