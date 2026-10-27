use vaegram::raw::{Deserializable, Error, LAYER, RemoteCall, Serializable, base, functions, types};

#[test]
fn request_bytes() {
    assert_eq!(functions::help::GetConfig {}.to_bytes(), [0x6b, 0x18, 0xf9, 0xc4]);

    let call = functions::InvokeWithLayer { layer: LAYER, query: functions::help::GetConfig {} };
    let b = call.to_bytes();
    assert_eq!(b[..4], 0xda9b0d0du32.to_le_bytes());
    assert_eq!(b[4..8], LAYER.to_le_bytes());
    assert_eq!(b[8..], [0x6b, 0x18, 0xf9, 0xc4]);

    let _: fn(<functions::InvokeWithLayer<functions::help::GetConfig> as RemoteCall>::Return) -> base::Config = |c| c;
}

#[test]
fn bytes_are_length_prefixed_and_padded() {
    assert_eq!("hi".to_string().to_bytes(), [2, b'h', b'i', 0]);

    let long = vec![7u8; 300];
    let b = long.to_bytes();
    assert_eq!(b[..4], [254, 44, 1, 0]);
    assert_eq!(b.len() % 4, 0);
    assert_eq!(Vec::<u8>::from_bytes(&b).unwrap(), long);
}

#[test]
fn round_trip_with_flags() {
    let with = |chosen_order| {
        base::ReactionCount::ReactionCount(Box::new(types::ReactionCount {
            chosen_order,
            reaction: base::Reaction::ReactionEmoji(Box::new(types::ReactionEmoji { emoticon: "👍".into() })),
            count: 42,
        }))
    };

    let b = with(Some(3)).to_bytes();
    assert_eq!(b[4..8], 1u32.to_le_bytes());
    assert_eq!(base::ReactionCount::from_bytes(&b).unwrap(), with(Some(3)));

    let b = with(None).to_bytes();
    assert_eq!(b[4..8], 0u32.to_le_bytes());
    assert_eq!(base::ReactionCount::from_bytes(&b).unwrap(), with(None));

    assert_eq!(base::ReactionCount::from_bytes(&b[..b.len() - 1]), Err(Error::UnexpectedEof));
    assert_eq!(base::Reaction::from_bytes(&[1, 2, 3, 4]), Err(Error::UnexpectedConstructor { id: 0x04030201 }));
}

#[test]
fn parses_containers() {
    use vaegram::raw::core::msg_container;

    let mut b = msg_container::ID.to_le_bytes().to_vec();
    b.extend(2u32.to_le_bytes());
    for (id, body) in [(5i64, &b"abcd"[..]), (9, b"12345678")] {
        b.extend(id.to_le_bytes());
        b.extend(1i32.to_le_bytes());
        b.extend((body.len() as u32).to_le_bytes());
        b.extend(body);
    }
    let messages = msg_container::parse(&b).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!((messages[1].msg_id, messages[1].body), (9, &b"12345678"[..]));

    assert!(msg_container::parse(&b[..b.len() - 1]).is_err());
}

#[test]
fn unpacks_gzip() {
    use std::io::Write;
    use vaegram::raw::core::gzip_packed;

    let inner = functions::help::GetConfig {}.to_bytes();
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(&inner).unwrap();
    let mut b = gzip_packed::ID.to_le_bytes().to_vec();
    gz.finish().unwrap().serialize(&mut b);

    assert_eq!(gzip_packed::unpack(&b).unwrap(), inner);
    assert_eq!(gzip_packed::unpack(&b[..8]), Err(Error::UnexpectedEof));
}

#[test]
fn updates_expose_pts_and_qts() {
    let deleted = base::Update::UpdateDeleteMessages(Box::new(types::UpdateDeleteMessages {
        messages: vec![1, 2],
        pts: 40,
        pts_count: 2,
    }));
    assert_eq!((deleted.pts(), deleted.qts()), (Some((40, 2)), None));

    let stopped = base::Update::UpdateBotStopped(Box::new(types::UpdateBotStopped {
        user_id: 1,
        date: 0,
        stopped: true,
        qts: 7,
    }));
    assert_eq!((stopped.pts(), stopped.qts()), (None, Some(7)));

    assert_eq!(base::Update::UpdateConfig.pts(), None);
}
