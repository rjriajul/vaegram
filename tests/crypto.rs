use vaegram::crypto::{prime, rsa::PublicKey};

#[test]
fn factorizes_pq() {
    assert_eq!(prime::factorize(0x17ED48941A08F981), Some((0x494C553B, 0x53911073)));
    assert_eq!(prime::factorize(15), Some((3, 5)));
    assert_eq!(prime::factorize(1_000_000_007), None);
}

#[test]
fn production_key_fingerprint() {
    assert_eq!(PublicKey::production().fingerprint(), 0xd09d1d85de64fd85u64 as i64);
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
