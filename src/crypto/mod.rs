pub mod aes;
pub mod mtproto;
pub mod prime;
pub mod rsa;

use num_bigint::BigUint;
use sha2::Digest;

pub(crate) fn sha1(parts: &[&[u8]]) -> [u8; 20] {
    let mut h = ::sha1::Sha1::new();
    parts.iter().for_each(|p| h.update(p));
    h.finalize().into()
}

pub(crate) fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = sha2::Sha256::new();
    parts.iter().for_each(|p| h.update(p));
    h.finalize().into()
}

pub(crate) fn random_fill(buf: &mut [u8]) {
    getrandom::getrandom(buf).expect("the OS random number generator failed");
}

pub(crate) fn random<const N: usize>() -> [u8; N] {
    let mut b = [0; N];
    random_fill(&mut b);
    b
}

pub(crate) fn to_be_padded<const N: usize>(x: &BigUint) -> [u8; N] {
    let b = x.to_bytes_be();
    let mut out = [0; N];
    out[N - b.len()..].copy_from_slice(&b);
    out
}
