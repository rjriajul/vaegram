use vaegram::crypto::{prime, rsa::PublicKey};

#[test]
fn factorizes_pq() {
    assert_eq!(prime::factorize(0x17ED48941A08F981), Some((0x494C553B, 0x53911073)));
    assert_eq!(prime::factorize(15), Some((3, 5)));
    assert_eq!(prime::factorize(1_000_000_007), None);
}

#[test]
fn server_key_fingerprints() {
    assert_eq!(PublicKey::production().fingerprint(), 0xd09d1d85de64fd85u64 as i64);
    assert_eq!(PublicKey::test().fingerprint(), 0xb25898df208d2603u64 as i64);
}

const PRODUCTION_PEM: &str = "-----BEGIN RSA PUBLIC KEY-----
MIIBCgKCAQEA6LszBcC1LGzyr992NzE0ieY+BSaOW622Aa9Bd4ZHLl+TuFQ4lo4g
5nKaMBwK/BIb9xUfg0Q29/2mgIR6Zr9krM7HjuIcCzFvDtr+L0GQjae9H0pRB2OO
62cECs5HKhT5DZ98K33vmWiLowc621dQuwKWSQKjWf50XYFw42h21P2KXUGyp2y/
+aEyZ+uVgLLQbRA1dEjSDZ2iGRy12Mk5gpYc397aYp438fsJoHIgJ2lgMv5h7WY9
t6N/byY9Nw9p21Og3AoXSL2q/2IJ1WRUhebgAdGVMlV1fkuOQoEzR7EdpqtQD9Cs
5+bfo3Nhmcyvk5ftB0WkJ9z6bNZ7yxrP8wIDAQAB
-----END RSA PUBLIC KEY-----";

const TEST_PEM: &str = "-----BEGIN RSA PUBLIC KEY-----
MIIBCgKCAQEAyMEdY1aR+sCR3ZSJrtztKTKqigvO/vBfqACJLZtS7QMgCGXJ6XIR
yy7mx66W0/sOFa7/1mAZtEoIokDP3ShoqF4fVNb6XeqgQfaUHd8wJpDWHcR2OFwv
plUUI1PLTktZ9uW2WE23b+ixNwJjJGwBDJPQEQFBE+vfmH0JP503wr5INS1poWg/
j25sIWeYPHYeOrFp/eXaqhISP6G+q2IeTaWTXpwZj4LzXq5YOpk4bYEQ6mvRq7D1
aHWfYmlEGepfaYR8Q0YqvvhYtMte3ITnuSJs171+GDqpdKcSwHnd6FudwGO4pcCO
j4WcDuXc2CTHgH8gFTNhp/Y8/SpDOhvn9QIDAQAB
-----END RSA PUBLIC KEY-----";

#[test]
fn reads_pem_keys() {
    let fingerprint = |pem: &str| PublicKey::from_pem(pem).map(|k| k.fingerprint() as u64);
    assert_eq!(fingerprint(PRODUCTION_PEM), Some(0xd09d1d85de64fd85));
    assert_eq!(fingerprint(TEST_PEM), Some(0xb25898df208d2603));
    assert_eq!(fingerprint(&PRODUCTION_PEM.replace("MIIBCg", "MIIBCw")), None);
    assert_eq!(fingerprint(&PRODUCTION_PEM.replace('A', "*")), None);
    assert_eq!(fingerprint(&PRODUCTION_PEM[..200]), None);
}

#[test]
fn rsa_pad_output_is_256_bytes_and_random() {
    let key = PublicKey::production();
    let (a, b) = (key.encrypt_pad(b"hello"), key.encrypt_pad(b"hello"));
    assert_eq!(a.len(), 256);
    assert_ne!(a, b);
}

#[test]
fn dh_prime_is_2048_bits() {
    assert_eq!(prime::dh_prime().bits(), 2048);
}
