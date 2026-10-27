// Fake-TLS for `ee` MTProxy secrets: a browser-like TLS ClientHello whose random carries an HMAC of
// the hello keyed by the secret, so the proxy can tell us from a real browser, then MTProto inside
// TLS application-data records.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::time::{SystemTime, UNIX_EPOCH};

use num_bigint::BigUint;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};

use crate::crypto::{hmac_sha256, random_fill};

pub(crate) const MAX_DOMAIN: usize = 182;
const MAX_RECORD: usize = 2878;
const CHANGE_CIPHER_SPEC: &[u8] = b"\x14\x03\x03\x00\x01\x01";
const APP_DATA: &[u8; 3] = b"\x17\x03\x03";
// Where the 32-byte random sits in both hellos; it carries the HMAC.
const RANDOM: std::ops::Range<usize> = 11..43;

pub(crate) async fn handshake<S: AsyncRead + AsyncWrite + Unpin>(
    s: &mut S,
    key: &[u8; 16],
    domain: &[u8],
) -> io::Result<()> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as u32);
    let hello = client_hello(key, domain, now, &mut random_fill);
    s.write_all(&hello).await?;

    let mut response = Vec::with_capacity(4096);
    for prefix in [&b"\x16\x03\x03"[..], b"\x14\x03\x03\x00\x01\x01\x17\x03\x03"] {
        let start = response.len();
        response.resize(start + prefix.len() + 2, 0);
        s.read_exact(&mut response[start..]).await?;
        if response[start..start + prefix.len()] != *prefix {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "MTProxy did not answer like a TLS server"));
        }
        let len = u16::from_be_bytes([response[response.len() - 2], response[response.len() - 1]]) as usize;
        let start = response.len();
        response.resize(start + len, 0);
        s.read_exact(&mut response[start..]).await?;
    }
    if !server_hello_matches(key, &hello[RANDOM], &mut response) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "MTProxy fake-TLS reply is not signed with the secret"));
    }
    Ok(())
}

// The server random is an HMAC over our random and its reply with that field zeroed.
fn server_hello_matches(key: &[u8; 16], client_random: &[u8], response: &mut [u8]) -> bool {
    let Some(field) = response.get_mut(RANDOM) else { return false };
    let mut got = [0u8; 32];
    got.copy_from_slice(field);
    field.fill(0);
    hmac_sha256(key, &[client_random, response]) == got
}

pub(crate) fn client_hello(key: &[u8; 16], domain: &[u8], unix_time: u32, fill: &mut impl FnMut(&mut [u8])) -> Vec<u8> {
    let mut grease = [0u8; 7];
    fill(&mut grease);
    for g in &mut grease {
        *g = (*g & 0xf0) | 0x0a;
    }
    for i in (1..grease.len()).step_by(2) {
        if grease[i] == grease[i - 1] {
            grease[i] ^= 0x10;
        }
    }
    let domain = &domain[..domain.len().min(MAX_DOMAIN)];
    let mut h = Hello { out: Vec::with_capacity(2048), grease, fill };
    h.put(b"\x16\x03\x01");
    h.scope(|h| {
        h.put(b"\x01\x00");
        h.scope(|h| {
            h.put(b"\x03\x03");
            h.put(&[0; 32]);
            h.put(b"\x20");
            h.random(32);
            h.put(b"\x00\x20");
            h.grease(0);
            h.put(b"\x13\x01\x13\x02\x13\x03\xc0\x2b\xc0\x2f\xc0\x2c\xc0\x30\xcc\xa9\xcc\xa8\xc0\x13\xc0\x14\x00\x9c");
            h.put(b"\x00\x9d\x00\x2f\x00\x35\x01\x00");
            h.scope(|h| {
                h.grease(2);
                h.put(b"\x00\x00");
                let mut parts: Vec<Vec<u8>> = (0..16).map(|i| h.part(|h| h.extension(i, domain))).collect();
                for i in (1..parts.len()).rev() {
                    let j = h.u32() as usize % (i + 1);
                    parts.swap(i, j);
                }
                parts.iter().for_each(|p| h.put(p));
                h.grease(3);
                h.put(b"\x00\x01\x00");
            });
        });
    });
    let mut hello = h.out;
    let digest = hmac_sha256(key, &[&hello]);
    hello[RANDOM].copy_from_slice(&digest);
    for (b, t) in hello[39..43].iter_mut().zip(unix_time.to_le_bytes()) {
        *b ^= t;
    }
    hello
}

struct Hello<'a, F> {
    out: Vec<u8>,
    grease: [u8; 7],
    fill: &'a mut F,
}

impl<F: FnMut(&mut [u8])> Hello<'_, F> {
    fn put(&mut self, bytes: &[u8]) {
        self.out.extend_from_slice(bytes);
    }

    fn random(&mut self, n: usize) {
        let start = self.out.len();
        self.out.resize(start + n, 0);
        (self.fill)(&mut self.out[start..]);
    }

    fn u32(&mut self) -> u32 {
        let mut b = [0; 4];
        (self.fill)(&mut b);
        u32::from_le_bytes(b)
    }

    fn grease(&mut self, i: usize) {
        self.put(&[self.grease[i]; 2]);
    }

    fn scope(&mut self, body: impl FnOnce(&mut Self)) {
        let start = self.out.len();
        self.put(&[0, 0]);
        body(self);
        let len = (self.out.len() - start - 2) as u16;
        self.out[start..start + 2].copy_from_slice(&len.to_be_bytes());
    }

    fn part(&mut self, body: impl FnOnce(&mut Self)) -> Vec<u8> {
        let outer = std::mem::replace(&mut self.out, Vec::with_capacity(64));
        body(self);
        std::mem::replace(&mut self.out, outer)
    }

    fn extension(&mut self, i: usize, domain: &[u8]) {
        match i {
            0 => {
                self.put(b"\x00\x00");
                self.scope(|h| {
                    h.scope(|h| {
                        h.put(b"\x00");
                        h.scope(|h| h.put(domain));
                    })
                });
            }
            1 => self.put(b"\x00\x05\x00\x05\x01\x00\x00\x00\x00"),
            2 => {
                self.put(b"\x00\x0a\x00\x0c\x00\x0a");
                self.grease(4);
                self.put(b"\x11\xec\x00\x1d\x00\x17\x00\x18");
            }
            3 => self.put(b"\x00\x0b\x00\x02\x01\x00"),
            4 => {
                self.put(b"\x00\x0d\x00\x18\x00\x16\x09\x04\x09\x05\x09\x06\x04\x03");
                self.put(b"\x08\x04\x04\x01\x05\x03\x08\x05\x05\x01\x08\x06\x06\x01");
            }
            5 => self.put(b"\x00\x10\x00\x0e\x00\x0c\x02h2\x08http/1.1"),
            6 => self.put(b"\x00\x12\x00\x00"),
            7 => self.put(b"\x00\x17\x00\x00"),
            8 => self.put(b"\x00\x1b\x00\x03\x02\x00\x02"),
            9 => self.put(b"\x00\x23\x00\x00"),
            10 => {
                self.put(b"\x00\x2b\x00\x07\x06");
                self.grease(6);
                self.put(b"\x03\x04\x03\x03");
            }
            11 => self.put(b"\x00\x2d\x00\x02\x01\x01"),
            12 => {
                self.put(b"\x00\x33\x04\xef\x04\xed");
                self.grease(4);
                self.put(b"\x00\x01\x00\x11\xec\x04\xc0");
                self.ml_kem_key();
                self.x25519_key();
                self.put(b"\x00\x1d\x00\x20");
                self.x25519_key();
            }
            13 => self.put(b"\x44\xcd\x00\x05\x00\x03\x02h2"),
            14 => {
                self.put(b"\xfe\x0d");
                self.scope(|h| {
                    h.put(b"\x00\x00\x01\x00\x01");
                    h.random(1);
                    h.put(b"\x00\x20");
                    h.x25519_key();
                    let mut size = [0];
                    (h.fill)(&mut size);
                    h.scope(|h| h.random(144 + 32 * (size[0] & 3) as usize));
                });
            }
            _ => self.put(b"\xff\x01\x00\x01\x00"),
        }
    }

    // Coefficients below q, packed 12 bits each, then a 32-byte seed.
    fn ml_kem_key(&mut self) {
        for _ in 0..384 {
            let a = self.u32() % 3329;
            let b = self.u32() % 3329;
            self.put(&[a as u8, (a >> 8) as u8 | (b << 4) as u8, (b >> 4) as u8]);
        }
        self.random(32);
    }

    // A real curve25519 point, so the key share survives DPI that checks it.
    fn x25519_key(&mut self) {
        let p = (BigUint::from(1u32) << 255u32) - 19u32;
        let one = BigUint::from(1u32);
        let y2 = |x: &BigUint| ((x + 486662u32) * x % &p + 1u32) * x % &p;
        let mut k = [0u8; 32];
        let mut x = loop {
            (self.fill)(&mut k);
            k[31] &= 127;
            let x = BigUint::from_bytes_be(&k);
            if y2(&x).modpow(&((&p - 1u32) >> 1u32), &p) == one {
                break x;
            }
        };
        for _ in 0..3 {
            let num = (&x * &x % &p + &p - 1u32) % &p;
            let den = y2(&x) * 4u32 % &p;
            x = &num * &num % &p * den.modpow(&(&p - 2u32), &p) % &p;
        }
        let le = x.to_bytes_le();
        k = [0; 32];
        k[..le.len()].copy_from_slice(&le);
        self.put(&k);
    }
}

pub(crate) struct RecordWriter {
    header: Option<[u8; 64]>,
    out: Vec<u8>,
}

impl RecordWriter {
    pub(crate) fn new(header: [u8; 64]) -> Self {
        RecordWriter { header: Some(header), out: Vec::new() }
    }

    pub(crate) fn wrap(&mut self, mut data: &[u8]) -> &[u8] {
        self.out.clear();
        if let Some(header) = self.header.take() {
            let n = data.len().min(MAX_RECORD - header.len());
            self.out.extend_from_slice(CHANGE_CIPHER_SPEC);
            self.record(&header, &data[..n]);
            data = &data[n..];
        }
        for chunk in data.chunks(MAX_RECORD) {
            self.record(&[], chunk);
        }
        &self.out
    }

    fn record(&mut self, head: &[u8], body: &[u8]) {
        self.out.extend_from_slice(APP_DATA);
        self.out.extend_from_slice(&((head.len() + body.len()) as u16).to_be_bytes());
        self.out.extend_from_slice(head);
        self.out.extend_from_slice(body);
    }
}

#[derive(Default)]
pub(crate) struct RecordReader {
    header: [u8; 5],
    got: usize,
    left: usize,
}

impl RecordReader {
    pub(crate) fn poll_read<R: AsyncRead + Unpin>(
        &mut self,
        r: &mut R,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        while self.left == 0 {
            while self.got < self.header.len() {
                let mut head = ReadBuf::new(&mut self.header[self.got..]);
                ready!(Pin::new(&mut *r).poll_read(cx, &mut head))?;
                if head.filled().is_empty() {
                    return Poll::Ready(Ok(()));
                }
                self.got += head.filled().len();
            }
            self.got = 0;
            if self.header[..3] != *APP_DATA {
                return Poll::Ready(Err(io::Error::new(io::ErrorKind::InvalidData, "MTProxy sent a non-TLS record")));
            }
            self.left = u16::from_be_bytes([self.header[3], self.header[4]]) as usize;
        }
        let n = self.left.min(buf.remaining());
        let mut body = ReadBuf::new(buf.initialize_unfilled_to(n));
        ready!(Pin::new(r).poll_read(cx, &mut body))?;
        let read = body.filled().len();
        buf.advance(read);
        self.left -= read;
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::sha256;

    const KEY: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    fn counter() -> impl FnMut(&mut [u8]) {
        let mut next = 0u8;
        move |buf| {
            for b in buf {
                *b = next;
                next = next.wrapping_add(1);
            }
        }
    }

    #[test]
    fn client_hello_matches_reference() {
        let hello = client_hello(&KEY, b"google.com", 0x5f5e1000, &mut counter());
        assert_eq!(hello.len(), 1726);
        assert_eq!(hello[..11], hex("16030106b9010006b50303"));
        assert_eq!(hello[11..43], hex("07e15754a41051a140f6fc1c50e37b2c3e4b4842f5c645f5ac65246365cb984f"));
        assert_eq!(sha256(&[&hello]).to_vec(), hex("1700213b919c96615ca2094114d3dae35a8ceacfa06edeed546cc77a437c44dc"));

        let long = client_hello(&KEY, &[b'a'; 200], 1, &mut counter());
        assert_eq!(long.len(), 1898);
        assert_eq!(sha256(&[&long]).to_vec(), hex("92964c3e6502ad1dab2c919bb4b0c28cba9381bdcebc853490e6fce591c6a878"));
    }

    #[test]
    fn server_hello_must_be_signed() {
        let client_random = &client_hello(&KEY, b"google.com", 0x5f5e1000, &mut counter())[11..43];
        let reply = hex(concat!(
            "1603030078020000760303a06fa4b9d4b0671ee96cc75af4c5de820e8cc138ae3bf28e6e0efedd38ea4691200001020304",
            "05060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f13010000000000000000000000000000000000000000",
            "000000000000000000000000000000000000000000000000000000140303000101170303006400010203040506070809",
            "0a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f30313233343536373839",
            "3a3b3c3d3e3f404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f60616263",
        ));
        assert!(server_hello_matches(&KEY, client_random, &mut reply.clone()));
        let mut tampered = reply.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(!server_hello_matches(&KEY, client_random, &mut tampered));
        assert!(!server_hello_matches(&[9; 16], client_random, &mut reply.clone()));
        assert!(!server_hello_matches(&KEY, client_random, &mut reply[..40].to_vec()));
    }

    #[tokio::test]
    async fn handshake_round_trip() {
        let (mut client, mut server) = tokio::io::duplex(4096);
        let proxy = async move {
            let mut head = [0u8; 5];
            server.read_exact(&mut head).await.unwrap();
            let mut hello = head.to_vec();
            hello.resize(5 + u16::from_be_bytes([head[3], head[4]]) as usize, 0);
            server.read_exact(&mut hello[5..]).await.unwrap();
            let digest: [u8; 32] = hello[11..43].try_into().unwrap();
            hello[11..43].fill(0);
            let expected = hmac_sha256(&KEY, &[&hello]);
            assert_eq!(digest[..28], expected[..28]);
            let time = u32::from_le_bytes(std::array::from_fn(|i| digest[28 + i] ^ expected[28 + i]));
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as u32;
            assert!(now.abs_diff(time) < 5);
            let mut reply = b"\x16\x03\x03\x00\x26\x02\x00\x00\x22\x03\x03".to_vec();
            reply.extend_from_slice(&[0; 32]);
            reply.extend_from_slice(b"\x14\x03\x03\x00\x01\x01\x17\x03\x03\x00\x02hi");
            let mac = hmac_sha256(&KEY, &[&digest, &reply]);
            reply[11..43].copy_from_slice(&mac);
            server.write_all(&reply).await.unwrap();
            server
        };
        let (result, _server) = tokio::join!(handshake(&mut client, &KEY, b"google.com"), proxy);
        result.unwrap();
    }

    #[test]
    fn records_carry_header_then_split_at_max_size() {
        let mut w = RecordWriter::new([0xaa; 64]);
        let mut first = hex("1403030001011703030043");
        first.extend_from_slice(&[0xaa; 64]);
        first.extend_from_slice(b"abc");
        assert_eq!(w.wrap(b"abc"), first);

        let big = vec![7u8; MAX_RECORD + 1];
        let out = w.wrap(&big).to_vec();
        assert_eq!(out.len(), 5 + MAX_RECORD + 5 + 1);
        assert_eq!(out[..5], hex("1703030b3e"));
        assert_eq!(out[5 + MAX_RECORD..5 + MAX_RECORD + 6], hex("170303000107"));

        let mut w = RecordWriter::new([0xaa; 64]);
        let out = w.wrap(&big).to_vec();
        assert_eq!(out[6..11], hex("1703030b3e"));
        assert_eq!(out[11 + MAX_RECORD..11 + MAX_RECORD + 5], hex("1703030041"));
        assert_eq!(out.len(), 6 + 5 + MAX_RECORD + 5 + 65);
    }

    #[tokio::test]
    async fn reader_strips_records_across_split_reads() {
        let (mut sender, mut wire) = tokio::io::duplex(64);
        let feed = async move {
            let bytes = hex("17030300036162631703030000170303000564656667681703");
            for chunk in bytes.chunks(3) {
                sender.write_all(chunk).await.unwrap();
                tokio::task::yield_now().await;
            }
            sender.write_all(b"\x03\x00\x01z").await.unwrap();
        };
        let read = async move {
            let mut records = RecordReader::default();
            let mut out = Vec::new();
            loop {
                let mut chunk = [0u8; 2];
                let mut buf = ReadBuf::new(&mut chunk);
                std::future::poll_fn(|cx| records.poll_read(&mut wire, cx, &mut buf)).await.unwrap();
                if buf.filled().is_empty() {
                    return out;
                }
                out.extend_from_slice(buf.filled());
            }
        };
        let ((), out) = tokio::join!(feed, read);
        assert_eq!(out, b"abcdefghz");
    }

    #[tokio::test]
    async fn reader_rejects_other_records() {
        let (mut sender, mut wire) = tokio::io::duplex(64);
        sender.write_all(b"\x15\x03\x03\x00\x02\x02\x28").await.unwrap();
        let mut chunk = [0u8; 8];
        let mut buf = ReadBuf::new(&mut chunk);
        let mut records = RecordReader::default();
        let err = std::future::poll_fn(|cx| records.poll_read(&mut wire, cx, &mut buf)).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
