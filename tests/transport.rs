use tokio::io::AsyncWriteExt;
use vaegram::Error;
use vaegram::connection::Transport;

async fn round_trip(transport: Transport, payload: &[u8]) -> Vec<u8> {
    let (mut client, mut server) = tokio::io::duplex(1 << 16);
    let mut frame = Vec::new();
    transport.encode(payload, &mut frame);
    client.write_all(&frame).await.unwrap();
    transport.read(&mut server).await.unwrap()
}

#[tokio::test]
async fn packets_round_trip_across_length_boundaries() {
    for transport in [Transport::Abridged, Transport::Intermediate] {
        for words in [0, 1, 0x7e, 0x7f, 0x80, 0x3000] {
            let payload: Vec<u8> = (0..words * 4).map(|i| i as u8).collect();
            assert_eq!(round_trip(transport, &payload).await, payload, "{transport:?} {words}");
        }
    }
}

#[tokio::test]
async fn padded_reads_strip_any_padding() {
    let encrypted: Vec<u8> = [1u8; 8].into_iter().chain((0..16 + 32).map(|i| i as u8)).collect();
    let mut plain = vec![0u8; 16];
    plain.extend_from_slice(&8u32.to_le_bytes());
    plain.extend_from_slice(b"abcd1234");
    for payload in [encrypted, plain] {
        assert_eq!(round_trip(Transport::PaddedIntermediate, &payload).await, payload);
        for pad in [0, 3, 4, 15] {
            let (mut client, mut server) = tokio::io::duplex(256);
            client.write_all(&((payload.len() + pad) as u32).to_le_bytes()).await.unwrap();
            client.write_all(&payload).await.unwrap();
            client.write_all(&[0xaa; 15][..pad]).await.unwrap();
            assert_eq!(Transport::PaddedIntermediate.read(&mut server).await.unwrap(), payload, "pad {pad}");
        }
    }
}

#[test]
fn frames_carry_the_length_header() {
    let mut frame = Vec::new();
    Transport::Abridged.encode(&[0; 0x7e * 4], &mut frame);
    assert_eq!((frame[0], frame.len()), (0x7e, 1 + 0x7e * 4));

    frame.clear();
    Transport::Abridged.encode(&[0; 0x7f * 4], &mut frame);
    assert_eq!(frame[..4], [0x7f, 0x7f, 0, 0]);

    frame.clear();
    Transport::Abridged.encode(&vec![0; 0x12345 * 4], &mut frame);
    assert_eq!(frame[..4], [0x7f, 0x45, 0x23, 0x01]);

    frame.clear();
    Transport::Intermediate.encode(b"abcd1234", &mut frame);
    assert_eq!((&frame[..4], frame.len()), (&[8, 0, 0, 0][..], 12));

    for _ in 0..64 {
        frame.clear();
        Transport::PaddedIntermediate.encode(b"abcd1234", &mut frame);
        let len = u32::from_le_bytes(frame[..4].try_into().unwrap()) as usize;
        assert_eq!(frame.len(), 4 + len);
        assert!((8..24).contains(&len));
        assert_eq!(&frame[4..12], b"abcd1234");
    }
}

#[tokio::test]
async fn error_codes_surface() {
    for transport in [Transport::Abridged, Transport::Intermediate, Transport::PaddedIntermediate] {
        let (mut client, mut server) = tokio::io::duplex(64);
        let mut frame = Vec::new();
        transport.encode(&(-404i32).to_le_bytes(), &mut frame);
        server.write_all(&frame).await.unwrap();
        assert!(matches!(transport.read(&mut client).await, Err(Error::Transport(404))), "{transport:?}");
    }
}

#[tokio::test]
async fn rejects_oversized_packets() {
    let (mut client, mut server) = tokio::io::duplex(64);
    server.write_all(&[0x7f, 0xff, 0xff, 0xff]).await.unwrap();
    assert!(matches!(Transport::Abridged.read(&mut client).await, Err(Error::Io(_))));
}
