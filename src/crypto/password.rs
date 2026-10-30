use num_bigint::BigUint;
use sha2::{Digest, Sha512};

use super::{prime, sha256, to_be_padded};

const PBKDF2_ITERATIONS: u32 = 100_000;

struct Hmac {
    inner: Sha512,
    outer: Sha512,
}

impl Hmac {
    fn new(key: &[u8]) -> Self {
        let mut k = [0u8; 128];
        if key.len() > k.len() {
            k[..64].copy_from_slice(&Sha512::digest(key));
        } else {
            k[..key.len()].copy_from_slice(key);
        }
        let pad = |byte: u8| {
            let mut h = Sha512::new();
            h.update(k.map(|b| b ^ byte));
            h
        };
        Hmac { inner: pad(0x36), outer: pad(0x5c) }
    }

    fn mac(&self, parts: &[&[u8]]) -> [u8; 64] {
        let mut inner = self.inner.clone();
        parts.iter().for_each(|p| inner.update(p));
        let mut outer = self.outer.clone();
        outer.update(inner.finalize());
        outer.finalize().into()
    }
}

pub(crate) fn pbkdf2_sha512(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 64] {
    let hmac = Hmac::new(password);
    let mut u = hmac.mac(&[salt, &1u32.to_be_bytes()]);
    let mut out = u;
    for _ in 1..iterations {
        u = hmac.mac(&[&u]);
        out.iter_mut().zip(u).for_each(|(o, x)| *o ^= x);
    }
    out
}

fn salted(data: &[u8], salt: &[u8]) -> [u8; 32] {
    sha256(&[salt, data, salt])
}

fn password_hash(password: &[u8], salt1: &[u8], salt2: &[u8]) -> BigUint {
    let ph1 = salted(&salted(password, salt1), salt2);
    BigUint::from_bytes_be(&salted(&pbkdf2_sha512(&ph1, salt1, PBKDF2_ITERATIONS), salt2))
}

fn in_safe_range(x: &BigUint, p: &BigUint) -> bool {
    let margin = BigUint::from(1u8) << (2048 - 64);
    *x > margin && *x < p - &margin
}

pub(crate) struct Srp<'a> {
    pub(crate) salt1: &'a [u8],
    pub(crate) salt2: &'a [u8],
    pub(crate) g: i32,
    pub(crate) p: &'a [u8],
    pub(crate) g_b: &'a [u8],
}

fn group(g: i32, p: &[u8]) -> Result<(BigUint, BigUint), &'static str> {
    let p = BigUint::from_bytes_be(p);
    if p != prime::dh_prime() {
        return Err("server sent an unknown password prime");
    }
    if !(2..=7).contains(&g) {
        return Err("password g is out of range");
    }
    Ok((BigUint::from(g as u32), p))
}

pub(crate) fn verifier(
    password: &[u8],
    salt1: &[u8],
    salt2: &[u8],
    g: i32,
    p: &[u8],
) -> Result<[u8; 256], &'static str> {
    let (g, p) = group(g, p)?;
    Ok(to_be_padded::<256>(&g.modpow(&password_hash(password, salt1, salt2), &p)))
}

impl Srp<'_> {
    pub(crate) fn check(&self, password: &[u8], a: &[u8; 256]) -> Result<([u8; 256], [u8; 32]), &'static str> {
        let (g, p) = group(self.g, self.p)?;
        let g_b = BigUint::from_bytes_be(self.g_b);
        if !in_safe_range(&g_b, &p) {
            return Err("srp_B is out of the safe range");
        }
        let a = BigUint::from_bytes_be(a);
        let g_a = g.modpow(&a, &p);
        if !in_safe_range(&g_a, &p) {
            return Err("srp A is out of the safe range");
        }

        let pad = to_be_padded::<256>;
        let (p_pad, g_pad, g_a_pad, g_b_pad) = (pad(&p), pad(&g), pad(&g_a), pad(&g_b));
        let x = password_hash(password, self.salt1, self.salt2);
        let k = BigUint::from_bytes_be(&sha256(&[&p_pad, &g_pad]));
        let u = BigUint::from_bytes_be(&sha256(&[&g_a_pad, &g_b_pad]));
        let kv = k * g.modpow(&x, &p) % &p;
        let t = (g_b + &p - kv) % &p;
        let k_a = sha256(&[&pad(&t.modpow(&(a + u * x), &p))]);

        let mut h_pg = sha256(&[&p_pad]);
        h_pg.iter_mut().zip(sha256(&[&g_pad])).for_each(|(h, g)| *h ^= g);
        let m1 = sha256(&[&h_pg, &sha256(&[self.salt1]), &sha256(&[self.salt2]), &g_a_pad, &g_b_pad, &k_a]);
        Ok((g_a_pad, m1))
    }
}

#[cfg(test)]
mod tests {
    use num_bigint::BigUint;

    use super::{Srp, password_hash, pbkdf2_sha512, verifier};
    use crate::crypto::{prime, random, sha256, to_be_padded};

    #[test]
    fn pbkdf2_matches_known_vectors() {
        let hex = |b: [u8; 64]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        assert_eq!(
            hex(pbkdf2_sha512(b"password", b"salt", 1)),
            "867f70cf1ade02cff3752599a3a53dc4af34c7a669815ae5d513554e1c8cf252\
             c02d470a285a0501bad999bfe943c08f050235d7d68b1da55e63f73b60a57fce"
        );
        assert_eq!(
            hex(pbkdf2_sha512(b"password", b"salt", 3)),
            "b6b07cb2cebf4ad84468391a543824fccffe0e0769dbe6bddf10a65673c4b648\
             e612d44918f9ce9a19a1294cf5140628084ba994c3b21a4ef4741220b811c633"
        );
    }

    #[test]
    fn server_accepts_the_password_proof() {
        let (salt1, salt2, password) = (b"salt one".as_slice(), b"salt two".as_slice(), b"hunter2".as_slice());
        let p = prime::dh_prime();
        let g = BigUint::from(3u8);
        let pad = to_be_padded::<256>;

        let v = g.modpow(&password_hash(password, salt1, salt2), &p);
        let k = BigUint::from_bytes_be(&sha256(&[&pad(&p), &pad(&g)]));
        let b = BigUint::from_bytes_be(&random::<256>());
        let g_b = (k * &v + g.modpow(&b, &p)) % &p;

        let srp = Srp { salt1, salt2, g: 3, p: &pad(&p), g_b: &pad(&g_b) };
        let server_expects = |g_a: &[u8; 256]| {
            let u = BigUint::from_bytes_be(&sha256(&[g_a, &pad(&g_b)]));
            let s = (BigUint::from_bytes_be(g_a) * v.modpow(&u, &p)).modpow(&b, &p);
            let mut h_pg = sha256(&[&pad(&p)]);
            h_pg.iter_mut().zip(sha256(&[&pad(&g)])).for_each(|(h, g)| *h ^= g);
            sha256(&[&h_pg, &sha256(&[salt1]), &sha256(&[salt2]), g_a, &pad(&g_b), &sha256(&[&pad(&s)])])
        };

        let (g_a, m1) = srp.check(password, &random()).unwrap();
        assert_eq!(m1, server_expects(&g_a));

        let (g_a, m1) = srp.check(b"wrong", &random()).unwrap();
        assert_ne!(m1, server_expects(&g_a));
    }

    #[test]
    fn verifier_is_what_the_server_checks_against() {
        let (salt1, salt2) = (b"salt one".as_slice(), b"salt two".as_slice());
        let p = to_be_padded::<256>(&prime::dh_prime());
        let v = verifier(b"hunter2", salt1, salt2, 3, &p).unwrap();
        let x = password_hash(b"hunter2", salt1, salt2);
        assert_eq!(BigUint::from_bytes_be(&v), BigUint::from(3u8).modpow(&x, &prime::dh_prime()));

        assert!(verifier(b"hunter2", salt1, salt2, 8, &p).is_err());
        let mut other = p;
        other[255] ^= 2;
        assert!(verifier(b"hunter2", salt1, salt2, 3, &other).is_err());
    }

    // m1 was computed independently in Python (hashlib) from the steps at core.telegram.org/api/srp.
    #[test]
    fn proof_matches_fixed_vector() {
        let p = prime::dh_prime();
        let a: [u8; 256] = std::array::from_fn(|i| i as u8);
        let b = BigUint::from_bytes_be(&std::array::from_fn::<u8, 256, _>(|i| 255 - i as u8));
        let g_b = to_be_padded::<256>(&BigUint::from(3u8).modpow(&b, &p));
        let srp = Srp { salt1: b"salt one", salt2: b"salt two", g: 3, p: &to_be_padded::<256>(&p), g_b: &g_b };

        let (g_a, m1) = srp.check(b"hunter2", &a).unwrap();
        let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        assert_eq!(hex(&sha256(&[&g_a])), "5a6121b88303c209ca550110abaf7a46068b0347e4b7904c3c3b9ea55e8de9cb");
        assert_eq!(hex(&m1), "10674de1c3b744ff8ea677d224ef7657d83487475c995750a2c7ad515e0e019c");
    }

    #[test]
    fn rejects_unsafe_parameters() {
        let p = to_be_padded::<256>(&prime::dh_prime());
        let g_b = to_be_padded::<256>(&BigUint::from(3u8).modpow(&BigUint::from(1u32 << 31), &prime::dh_prime()));
        let a = [7u8; 256];
        let fails = |g, p: &[u8], g_b: &[u8]| Srp { salt1: b"s", salt2: b"t", g, p, g_b }.check(b"pw", &a).is_err();
        assert!(!fails(3, &p, &g_b));
        let mut other = p;
        other[255] ^= 2;
        let tiny = to_be_padded::<256>(&BigUint::from(2u8));
        let mut big = p;
        big[255] -= 1;
        let cases: [(i32, &[u8], &[u8], &str); 6] = [
            (1, &p, &g_b, "g = 1"),
            (8, &p, &g_b, "g = 8"),
            (3, &other, &g_b, "unknown prime"),
            (3, &p, &tiny, "tiny g_b"),
            (3, &p, &big, "g_b = p - 1"),
            (3, &p, &[0; 256], "zero g_b"),
        ];
        for (g, p, g_b, why) in cases {
            assert!(fails(g, p, g_b), "{why}");
        }
    }
}
