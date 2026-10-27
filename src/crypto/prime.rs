use num_bigint::BigUint;

/// The 2048-bit safe prime Telegram's servers use for every auth key exchange.
/// A server offering any other prime is rejected.
const DH_PRIME_HEX: &str = concat!(
    "C71CAEB9C6B1C9048E6C522F70F13F73980D40238E3E21C14934D037563D930F",
    "48198A0AA7C14058229493D22530F4DBFA336F6E0AC925139543AED44CCE7C37",
    "20FD51F69458705AC68CD4FE6B6B13ABDC9746512969328454F18FAF8C595F64",
    "2477FE96BB2A941D5BCD1D4AC8CC49880708FA9B378E3C4F3A9060BEE67CF9A4",
    "A4A695811051907E162753B56B0F6B410DBA74D8A84B2A14B3144E0EF1284754",
    "FD17ED950D5965B4B9DD46582DB1178D169C6BC465B0D6FF9CA3928FEF5B9AE4",
    "E418FC15E83EBEA0F87FA9FF5EED70050DED2849F47BF959D956850CE929851F",
    "0D8115F635B105EE2E4E15D04B2454BF6F4FADF034B10403119CD8E3B92FCC5B",
);

pub fn dh_prime() -> BigUint {
    BigUint::parse_bytes(DH_PRIME_HEX.as_bytes(), 16).unwrap()
}

pub fn factorize(pq: u64) -> Option<(u64, u64)> {
    if pq < 4 {
        return None;
    }
    if pq % 2 == 0 {
        return Some((2, pq / 2));
    }
    let n = pq as u128;
    for c in 1..64u128 {
        let f = |x: u64| ((x as u128 * x as u128 + c) % n) as u64;
        let (mut x, mut y, mut d) = (2u64, 2u64, 1u64);
        while d == 1 {
            x = f(x);
            y = f(f(y));
            d = gcd(x.abs_diff(y), pq);
        }
        if d != pq {
            let (a, b) = (d, pq / d);
            return Some((a.min(b), a.max(b)));
        }
    }
    None
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
