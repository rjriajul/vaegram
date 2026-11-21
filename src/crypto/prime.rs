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

/// Whether `g` generates the cyclic subgroup of prime order (p-1)/2 of the safe prime `p`, by the residue of `p`
/// that https://core.telegram.org/mtproto/auth_key asks clients to check for each `g` from 2 to 7.
pub fn generates_subgroup(g: i32, p: &BigUint) -> bool {
    let r = |m: u32| (p % m).to_u32_digits().first().copied().unwrap_or(0);
    match g {
        2 => r(8) == 7,
        3 => r(3) == 2,
        4 => true,
        5 => matches!(r(5), 1 | 4),
        6 => matches!(r(24), 19 | 23),
        7 => matches!(r(7), 3 | 5 | 6),
        _ => false,
    }
}

/// Most steps Pollard's rho may take across all attempts. A real pq splits into two ~32-bit
/// primes in about 2^16 steps; a prime pq from a hostile server would otherwise spin for hours.
const MAX_STEPS: u32 = 1 << 20;

/// Splits `pq` into the two distinct primes `p < q` the spec promises; anything else is `None`.
pub fn factorize(pq: u64) -> Option<(u64, u64)> {
    let (p, q) = split(pq)?;
    (p < q && is_prime(p) && is_prime(q)).then_some((p, q))
}

fn split(pq: u64) -> Option<(u64, u64)> {
    if pq < 4 {
        return None;
    }
    if pq % 2 == 0 {
        return Some((2, pq / 2));
    }
    let n = pq as u128;
    let mut steps = 0;
    for c in 1..64u128 {
        let f = |x: u64| ((x as u128 * x as u128 + c) % n) as u64;
        let (mut x, mut y, mut d) = (2u64, 2u64, 1u64);
        while d == 1 {
            steps += 1;
            if steps > MAX_STEPS {
                return None;
            }
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

// Miller-Rabin with the first twelve primes as bases, which is deterministic for every u64.
fn is_prime(n: u64) -> bool {
    const BASES: [u64; 12] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];
    if n < 2 {
        return false;
    }
    for b in BASES {
        if n % b == 0 {
            return n == b;
        }
    }
    let (mut d, mut s) = (n - 1, 0);
    while d % 2 == 0 {
        d /= 2;
        s += 1;
    }
    let mul = |a: u64, b: u64| (a as u128 * b as u128 % n as u128) as u64;
    'bases: for b in BASES {
        let (mut x, mut base, mut e) = (1, b, d);
        while e > 0 {
            if e & 1 == 1 {
                x = mul(x, base);
            }
            base = mul(base, base);
            e >>= 1;
        }
        if x == 1 || x == n - 1 {
            continue;
        }
        for _ in 1..s {
            x = mul(x, x);
            if x == n - 1 {
                continue 'bases;
            }
        }
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_a_real_pq() {
        assert_eq!(factorize(0x17ED48941A08F981), Some((0x494C553B, 0x53911073)));
        assert_eq!(factorize(15), Some((3, 5)));
    }

    #[test]
    fn gives_up_on_a_large_prime() {
        assert_eq!(factorize(18_446_744_073_709_551_557), None);
        assert_eq!(factorize(1_000_000_007), None);
    }

    #[test]
    fn refuses_anything_but_two_distinct_primes() {
        for pq in [4, 8, 9, 25, 105, 0x494C553B * 0x494C553B, 3 * 0x17ED48941A08F981] {
            assert_eq!(factorize(pq), None, "{pq}");
        }
        assert_eq!(factorize(6), Some((2, 3)));
    }

    #[test]
    fn tells_primes_from_composites() {
        let primes = [2, 3, 37, 41, 0x494C553B, 0x53911073, 1_000_000_007, 18_446_744_073_709_551_557];
        assert!(primes.into_iter().all(is_prime));
        // 3215031751 fools bases 2, 3, 5 and 7; 3825123056546413051 fools every base up to 31.
        let composites = [0, 1, 4, 9, 105, 3_215_031_751, 3_825_123_056_546_413_051, 0x17ED48941A08F981];
        assert!(!composites.into_iter().any(is_prime));
    }

    #[test]
    fn checks_the_generator_against_the_prime() {
        let ok = |g: i32, p: u32| generates_subgroup(g, &BigUint::from(p));
        // (g, p that passes, p that fails), each p a small safe prime.
        let cases = [(2, 23, 11), (3, 23, 7), (5, 11, 23), (6, 23, 11), (7, 47, 23)];
        for (g, good, bad) in cases {
            assert!(ok(g, good), "g = {g}, p = {good}");
            assert!(!ok(g, bad), "g = {g}, p = {bad}");
        }
        assert!(ok(4, 11) && ok(4, 23));
        assert!(!ok(1, 23) && !ok(8, 23) && !ok(-1, 23));

        // Telegram's prime is 3 mod 8, 2 mod 3, 3 mod 5, 11 mod 24 and 6 mod 7: it sends g = 3.
        let p = dh_prime();
        assert!([3, 4, 7].iter().all(|&g| generates_subgroup(g, &p)));
        assert!(![2, 5, 6].iter().any(|&g| generates_subgroup(g, &p)));
    }

    #[test]
    fn dh_prime_is_2048_bits() {
        assert_eq!(dh_prime().bits(), 2048);
    }
}
