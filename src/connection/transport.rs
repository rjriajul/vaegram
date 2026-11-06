use std::io;

use crate::crypto::random;
use crate::errors::{Error, Result};

const MAX_PACKET: usize = 16 << 20;

/// Read bytes; taken packets leave the front only once they fill half the buffer, so each byte moves once.
#[derive(Debug, Default)]
pub struct RecvBuf {
    data: Vec<u8>,
    start: usize,
    #[cfg(test)]
    moved: usize,
}

impl RecvBuf {
    /// Appends received bytes after the unread ones.
    pub fn extend_from_slice(&mut self, bytes: &[u8]) {
        self.make_room().extend_from_slice(bytes);
    }

    /// The bytes received but not yet taken as a packet.
    pub fn unread(&self) -> &[u8] {
        &self.data[self.start..]
    }

    pub(crate) fn make_room(&mut self) -> &mut Vec<u8> {
        if self.start == self.data.len() {
            self.data.clear();
            self.start = 0;
        } else if self.start >= self.data.len() / 2 {
            #[cfg(test)]
            {
                self.moved += self.data.len() - self.start;
            }
            self.data.drain(..self.start);
            self.start = 0;
        }
        &mut self.data
    }
}

/// MTProto transport framing, see <https://core.telegram.org/mtproto/mtproto-transports>.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Transport {
    /// One-byte length prefix for packets under 508 bytes, four bytes above that.
    Abridged,
    /// A four-byte length prefix before every packet. The default.
    #[default]
    Intermediate,
    /// Intermediate with random padding per packet; required by `dd` MTProxy secrets.
    PaddedIntermediate,
    /// Length, sequence number, payload and a CRC32 of all three before it, so a corrupted or lost packet drops the
    /// connection. Sends nothing before the first packet, so it can't be obfuscated or go through an MTProxy.
    Full,
}

impl Transport {
    /// The bytes that tell the server which transport follows; the full transport has none.
    pub(crate) fn tag(self) -> Option<[u8; 4]> {
        match self {
            Transport::Abridged => Some([0xef; 4]),
            Transport::Intermediate => Some([0xee; 4]),
            Transport::PaddedIntermediate => Some([0xdd; 4]),
            Transport::Full => None,
        }
    }
}

/// The state of one connection's framing: the transport, and for the full transport the packets sent and received
/// so far, which number them. Make a new one for every connection.
#[derive(Clone, Copy, Debug)]
pub struct Framing {
    transport: Transport,
    sent: u32,
    received: u32,
}

impl Framing {
    /// Free bytes [`Framing::frame`] needs in front of the payload.
    pub const ROOM: usize = 8;

    /// Fresh framing for a new connection: packet counters at zero.
    pub fn new(transport: Transport) -> Framing {
        Framing { transport, sent: 0, received: 0 }
    }

    /// Frames the payload in `buf[Self::ROOM..]` in place and returns where the frame starts.
    pub fn frame(&mut self, buf: &mut Vec<u8>) -> usize {
        self.frame_after(buf, 0)
    }

    /// Like [`Framing::frame`], with `skip` more bytes in front of the room, kept free for a layer below.
    pub(crate) fn frame_after(&mut self, buf: &mut Vec<u8>, skip: usize) -> usize {
        let len = buf.len() - skip - Self::ROOM;
        match self.transport {
            Transport::Abridged => {
                debug_assert!(len % 4 == 0 && len / 4 <= 0xff_ffff);
                let words = len / 4;
                if words < 0x7f {
                    buf[skip + 7] = words as u8;
                    skip + 7
                } else {
                    buf[skip + 4] = 0x7f;
                    buf[skip + 5..skip + 8].copy_from_slice(&(words as u32).to_le_bytes()[..3]);
                    skip + 4
                }
            }
            Transport::Intermediate => {
                buf[skip + 4..skip + 8].copy_from_slice(&(len as u32).to_le_bytes());
                skip + 4
            }
            Transport::PaddedIntermediate => {
                let noise: [u8; 16] = random();
                let pad = (noise[0] & 15) as usize;
                buf.extend_from_slice(&noise[1..1 + pad]);
                buf[skip + 4..skip + 8].copy_from_slice(&((len + pad) as u32).to_le_bytes());
                skip + 4
            }
            Transport::Full => {
                buf[skip..skip + 4].copy_from_slice(&(len as u32 + FULL_OVERHEAD as u32).to_le_bytes());
                buf[skip + 4..skip + 8].copy_from_slice(&self.sent.to_le_bytes());
                self.sent = self.sent.wrapping_add(1);
                let crc = crc32fast::hash(&buf[skip..]);
                buf.extend_from_slice(&crc.to_le_bytes());
                skip
            }
        }
    }

    /// Takes one whole packet off the front of `buf`, or `None` while it is still arriving.
    pub fn decode(&mut self, recv: &mut RecvBuf) -> Result<Option<Vec<u8>>> {
        if self.transport == Transport::Full {
            return self.decode_full(recv);
        }
        let buf = recv.unread();
        let (head, len) = match (self.transport, buf.first()) {
            (_, None) => return Ok(None),
            (Transport::Abridged, Some(&words)) if words != 0x7f => (1, words as usize * 4),
            _ if buf.len() < 4 => return Ok(None),
            (Transport::Abridged, _) => (4, u32::from_le_bytes([buf[1], buf[2], buf[3], 0]) as usize * 4),
            _ => (4, u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize),
        };
        if len > MAX_PACKET {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("{len}-byte packet is too big")).into());
        }
        if buf.len() < head + len {
            return Ok(None);
        }
        let mut payload = buf[head..head + len].to_vec();
        recv.start += head + len;
        if self.transport == Transport::PaddedIntermediate {
            payload.truncate(unpadded_len(&payload));
        }
        server_error(payload)
    }

    fn decode_full(&mut self, recv: &mut RecvBuf) -> Result<Option<Vec<u8>>> {
        let buf = recv.unread();
        let Some(head) = buf.first_chunk::<4>() else { return Ok(None) };
        let len = u32::from_le_bytes(*head) as usize;
        if !(FULL_OVERHEAD..=MAX_PACKET + FULL_OVERHEAD).contains(&len) {
            let message = format!("a full packet can't be {len} bytes long");
            return Err(io::Error::new(io::ErrorKind::InvalidData, message).into());
        }
        if buf.len() < len {
            return Ok(None);
        }
        let (packet, crc) = buf[..len].split_at(len - 4);
        if crc32fast::hash(packet).to_le_bytes() != *crc {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "a full packet failed its CRC check").into());
        }
        let seq = u32::from_le_bytes([packet[4], packet[5], packet[6], packet[7]]);
        if seq != self.received {
            let message = format!("a full packet came with sequence number {seq}, not {}", self.received);
            return Err(io::Error::new(io::ErrorKind::InvalidData, message).into());
        }
        let payload = packet[8..].to_vec();
        self.received = self.received.wrapping_add(1);
        recv.start += len;
        server_error(payload)
    }
}

/// Length, sequence number and CRC around a full packet's payload.
const FULL_OVERHEAD: usize = 12;

/// A 4-byte negative payload is the server refusing the connection.
fn server_error(payload: Vec<u8>) -> Result<Option<Vec<u8>>> {
    if let Ok(code) = <[u8; 4]>::try_from(&payload[..]).map(i32::from_le_bytes) {
        if code < 0 {
            return Err(Error::Transport(code.saturating_neg()));
        }
    }
    Ok(Some(payload))
}

// Padding is 0-15 bytes and not marked, so the real length comes from the MTProto packet itself:
// error codes are 4 bytes, plain messages carry their body length, encrypted ones are 24 + 16n.
fn unpadded_len(p: &[u8]) -> usize {
    if p.len() < 24 {
        return p.len().min(4);
    }
    if p[..8] == [0; 8] {
        let body = u32::from_le_bytes([p[16], p[17], p[18], p[19]]) as usize;
        return p.len().min(body.saturating_add(20));
    }
    24 + (p.len() - 24) / 16 * 16
}

#[cfg(test)]
mod tests {
    use super::*;

    fn framed_by(framing: &mut Framing, payload: &[u8]) -> Vec<u8> {
        let mut buf = vec![0; Framing::ROOM];
        buf.extend_from_slice(payload);
        let start = framing.frame(&mut buf);
        buf.drain(..start);
        buf
    }

    fn framed(transport: Transport, payload: &[u8]) -> Vec<u8> {
        framed_by(&mut Framing::new(transport), payload)
    }

    fn trickle(transport: Transport, frame: &[u8]) -> Result<Vec<u8>> {
        let (mut framing, mut buf) = (Framing::new(transport), RecvBuf::default());
        for (i, &b) in frame.iter().enumerate() {
            buf.extend_from_slice(&[b]);
            if let Some(packet) = framing.decode(&mut buf)? {
                assert_eq!(i, frame.len() - 1, "{transport:?} returned a packet early");
                assert!(buf.unread().is_empty());
                return Ok(packet);
            }
        }
        panic!("{transport:?} never returned the packet");
    }

    fn invalid(read: Result<Option<Vec<u8>>>) -> bool {
        matches!(read, Err(Error::Io(e)) if e.kind() == io::ErrorKind::InvalidData)
    }

    #[test]
    fn packets_round_trip_across_length_boundaries() {
        for transport in [Transport::Abridged, Transport::Intermediate, Transport::Full] {
            for words in [0, 1, 0x7e, 0x7f, 0x80, 0x3000] {
                let payload: Vec<u8> = (0..words * 4).map(|i| i as u8).collect();
                let packet = trickle(transport, &framed(transport, &payload)).unwrap();
                assert_eq!(packet[..], payload[..], "{transport:?} {words}");
            }
        }
    }

    #[test]
    fn several_packets_in_one_read() {
        let (mut tx, mut rx) = (Framing::new(Transport::Intermediate), Framing::new(Transport::Intermediate));
        let mut buf = RecvBuf::default();
        for i in 0..3u8 {
            buf.extend_from_slice(&framed_by(&mut tx, &[i; 8]));
        }
        buf.extend_from_slice(&[8, 0]);
        for i in 0..3u8 {
            assert_eq!(rx.decode(&mut buf).unwrap().unwrap()[..], [i; 8]);
        }
        assert!(rx.decode(&mut buf).unwrap().is_none());
        assert_eq!(buf.unread(), [8, 0]);
    }

    #[test]
    fn full_frames_round_trip_with_sequence_numbers() {
        let (mut tx, mut rx) = (Framing::new(Transport::Full), Framing::new(Transport::Full));
        let frames: Vec<Vec<u8>> = (0..3u8).map(|i| framed_by(&mut tx, &[i; 8])).collect();
        // Length with its own 12 bytes, the sequence number, the payload, then the CRC of all of that.
        for (i, frame) in frames.iter().enumerate() {
            assert_eq!(frame[..4], 20u32.to_le_bytes());
            assert_eq!(frame[4..8], (i as u32).to_le_bytes());
            assert_eq!(frame[8..16], [i as u8; 8]);
            assert_eq!(frame[16..], crc32fast::hash(&frame[..16]).to_le_bytes());
        }
        let mut buf = RecvBuf::default();
        for frame in &frames {
            buf.extend_from_slice(frame);
        }
        buf.extend_from_slice(&[20, 0]);
        for i in 0..3u8 {
            assert_eq!(rx.decode(&mut buf).unwrap().unwrap()[..], [i; 8]);
        }
        assert!(rx.decode(&mut buf).unwrap().is_none());
        assert_eq!(buf.unread(), [20, 0]);
    }

    #[test]
    fn a_bad_crc_or_sequence_drops_the_connection() {
        let frame = framed(Transport::Full, b"abcd1234");
        let decode = |framing: &mut Framing, bytes: &[u8]| {
            let mut buf = RecvBuf::default();
            buf.extend_from_slice(bytes);
            framing.decode(&mut buf)
        };
        assert!(decode(&mut Framing::new(Transport::Full), &frame).unwrap().is_some());

        for at in [8, 15, 16, 19] {
            let mut bad = frame.clone();
            bad[at] ^= 1;
            assert!(invalid(decode(&mut Framing::new(Transport::Full), &bad)), "byte {at}");
        }
        // A packet repeated or skipped: the CRC is fine, the number isn't.
        let mut rx = Framing::new(Transport::Full);
        assert!(decode(&mut rx, &frame).unwrap().is_some());
        assert!(invalid(decode(&mut rx, &frame)), "a repeated packet");
        let mut skipped = Framing::new(Transport::Full);
        let second = framed_by(&mut Framing { sent: 1, ..skipped }, b"abcd1234");
        assert!(invalid(decode(&mut skipped, &second)), "a skipped packet");
        // Lengths that can't be a packet.
        for len in [0u32, 11, 16 << 20 | 13] {
            assert!(invalid(decode(&mut Framing::new(Transport::Full), &len.to_le_bytes())), "length {len}");
        }
    }

    #[test]
    fn padded_reads_strip_any_padding() {
        let encrypted: Vec<u8> = [1u8; 8].into_iter().chain((0..16 + 32).map(|i| i as u8)).collect();
        let mut plain = vec![0u8; 16];
        plain.extend_from_slice(&8u32.to_le_bytes());
        plain.extend_from_slice(b"abcd1234");
        for payload in [encrypted, plain] {
            for _ in 0..16 {
                let packet = trickle(Transport::PaddedIntermediate, &framed(Transport::PaddedIntermediate, &payload));
                assert_eq!(packet.unwrap()[..], payload[..]);
            }
            for pad in [0, 3, 4, 15] {
                let mut frame = ((payload.len() + pad) as u32).to_le_bytes().to_vec();
                frame.extend_from_slice(&payload);
                frame.extend_from_slice(&[0xaa; 15][..pad]);
                assert_eq!(trickle(Transport::PaddedIntermediate, &frame).unwrap()[..], payload[..], "pad {pad}");
            }
        }
    }

    #[test]
    fn frame_headers() {
        let frame = framed(Transport::Abridged, &[0; 0x7e * 4]);
        assert_eq!((frame[0], frame.len()), (0x7e, 1 + 0x7e * 4));
        assert_eq!(framed(Transport::Abridged, &[0; 0x7f * 4])[..4], [0x7f, 0x7f, 0, 0]);
        assert_eq!(framed(Transport::Abridged, &vec![0; 0x12345 * 4])[..4], [0x7f, 0x45, 0x23, 0x01]);
        assert_eq!(framed(Transport::Intermediate, b"abcd1234")[..], *b"\x08\0\0\0abcd1234");

        for _ in 0..64 {
            let mut frame = vec![0; Framing::ROOM];
            frame.extend_from_slice(b"abcd1234");
            assert_eq!(Framing::new(Transport::PaddedIntermediate).frame(&mut frame), 4);
            let len = u32::from_le_bytes(frame[4..8].try_into().unwrap()) as usize;
            assert_eq!(frame.len(), 8 + len);
            assert!((8..24).contains(&len));
            assert_eq!(&frame[8..16], b"abcd1234");
        }
    }

    #[test]
    fn error_codes_surface() {
        for transport in [Transport::Abridged, Transport::Intermediate, Transport::PaddedIntermediate, Transport::Full]
        {
            let frame = framed(transport, &(-404i32).to_le_bytes());
            assert!(matches!(trickle(transport, &frame), Err(Error::Transport(404))), "{transport:?}");
        }
    }

    #[test]
    fn rejects_oversized_packets() {
        let mut buf = RecvBuf::default();
        buf.extend_from_slice(&[0x7f, 0xff, 0xff, 0xff]);
        assert!(matches!(Framing::new(Transport::Abridged).decode(&mut buf), Err(Error::Io(_))));
    }

    #[test]
    fn a_burst_of_packets_moves_each_byte_at_most_once() {
        for transport in [Transport::Intermediate, Transport::Full] {
            let (mut tx, mut rx) = (Framing::new(transport), Framing::new(transport));
            let mut buf = RecvBuf::default();
            let mut total = 0;
            for _ in 0..200 {
                let burst: Vec<u8> = (0..9).flat_map(|_| framed_by(&mut tx, &[7; 1000])).collect();
                for chunk in burst.chunks(4096) {
                    buf.extend_from_slice(chunk);
                    total += chunk.len();
                    while let Some(packet) = rx.decode(&mut buf).unwrap() {
                        assert_eq!(packet.len(), 1000);
                    }
                }
            }
            assert!(buf.unread().is_empty());
            assert!(buf.moved <= total, "{transport:?}: {} bytes moved for {total} read", buf.moved);
        }
    }

    #[test]
    fn the_most_negative_error_code_does_not_overflow() {
        let frame = [4u32.to_le_bytes(), i32::MIN.to_le_bytes()].concat();
        let read = trickle(Transport::Intermediate, &frame);
        assert!(matches!(read, Err(Error::Transport(i32::MAX))), "{read:?}");
    }
}
