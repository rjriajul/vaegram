use std::collections::HashMap;

use vaegram::raw::{base, types};
use vaegram::session::Salt;
use vaegram::storage::state::{self, Peers, UpdateState};
use vaegram::storage::{SessionData, session_string, write_private};

const V3_STRING: &str = "VAE_AwQAADA5AAABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4fICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj9AQUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVpbXF1eX2BhYmNkZWZnaGlqa2xtbm9wcXJzdHV2d3h5ent8fX5_gIGCg4SFhoeIiYqLjI2Oj5CRkpOUlZaXmJmam5ydnp-goaKjpKWmp6ipqqusra6vsLGys7S1tre4ubq7vL2-v8DBwsPExcbHyMnKy8zNzs_Q0dLT1NXW19jZ2tvc3d7f4OHi4-Tl5ufo6err7O3u7_Dx8vP09fb3-Pn6-_z9_v8AAAAAAAvbKAEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAYNzrA";
const LEGACY_STRING: &str = "BAAAMDkAAAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8gISIjJCUmJygpKissLS4vMDEyMzQ1Njc4OTo7PD0-P0BBQkNERUZHSElKS0xNTk9QUVJTVFVWV1hZWltcXV5fYGFiY2RlZmdoaWprbG1ub3BxcnN0dXZ3eHl6e3x9fn-AgYKDhIWGh4iJiouMjY6PkJGSk5SVlpeYmZqbnJ2en6ChoqOkpaanqKmqq6ytrq-wsbKztLW2t7i5uru8vb6_wMHCw8TFxsfIycrLzM3Oz9DR0tPU1dbX2Nna29zd3t_g4eLj5OXm5-jp6uvs7e7v8PHy8_T19vf4-fr7_P3-_wAAAAAAC9soAQ";
const V4_STRING: &str = "VAE_BAQAADA5AQABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4fICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj9AQUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVpbXF1eX2BhYmNkZWZnaGlqa2xtbm9wcXJzdHV2d3h5ent8fX5_gIGCg4SFhoeIiYqLjI2Oj5CRkpOUlZaXmJmam5ydnp-goaKjpKWmp6ipqqusra6vsLGys7S1tre4ubq7vL2-v8DBwsPExcbHyMnKy8zNzs_Q0dLT1NXW19jZ2tvc3d7f4OHi4-Tl5ufo6err7O3u7_Dx8vP09fb3-Pn6-_z9_v8AAAAAAAvbKAEBuwAAAAAAAAAAAAD__5Wapyg2SpWJ";
// Each one is the real export_session_string of wzgram's storage.py (rjriajul/wzgram) at the commit
// that wrote that layout: WZ_V3 2d66ac8, WZ_V2 5a25d02, WZ_V2_NO_CRC e44599e, LEGACY_NO_API_ID fe764e0
// (the 64-bit user id branch), LEGACY_U32_USER 7dda167.
const WZ_V3: &str = "WZ_AwQAADA5AAABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4fICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj9AQUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVpbXF1eX2BhYmNkZWZnaGlqa2xtbm9wcXJzdHV2d3h5ent8fX5_gIGCg4SFhoeIiYqLjI2Oj5CRkpOUlZaXmJmam5ydnp-goaKjpKWmp6ipqqusra6vsLGys7S1tre4ubq7vL2-v8DBwsPExcbHyMnKy8zNzs_Q0dLT1NXW19jZ2tvc3d7f4OHi4-Tl5ufo6err7O3u7_Dx8vP09fb3-Pn6-_z9_v8AAAAAAAvbKAEBuzIwMDE6NjdjOjRlODpmMDA0OjpiAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAIyBZ_4";
const WZ_V3_NO_ADDR: &str = "WZ_AwQAADA5AQABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4fICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj9AQUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVpbXF1eX2BhYmNkZWZnaGlqa2xtbm9wcXJzdHV2d3h5ent8fX5_gIGCg4SFhoeIiYqLjI2Oj5CRkpOUlZaXmJmam5ydnp-goaKjpKWmp6ipqqusra6vsLGys7S1tre4ubq7vL2-v8DBwsPExcbHyMnKy8zNzs_Q0dLT1NXW19jZ2tvc3d7f4OHi4-Tl5ufo6err7O3u7_Dx8vP09fb3-Pn6-_z9_v8AAAAAAAvbKAEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABwhHEg";
const WZ_V2: &str = "WZ_AgQAADA5AAABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4fICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj9AQUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVpbXF1eX2BhYmNkZWZnaGlqa2xtbm9wcXJzdHV2d3h5ent8fX5_gIGCg4SFhoeIiYqLjI2Oj5CRkpOUlZaXmJmam5ydnp-goaKjpKWmp6ipqqusra6vsLGys7S1tre4ubq7vL2-v8DBwsPExcbHyMnKy8zNzs_Q0dLT1NXW19jZ2tvc3d7f4OHi4-Tl5ufo6err7O3u7_Dx8vP09fb3-Pn6-_z9_v8AAAAAAAvbKAEBuzE0OS4xNTQuMTY3LjkxAACU96HB";
const WZ_V2_NO_CRC: &str = "AgQAADA5AAABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4fICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj9AQUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVpbXF1eX2BhYmNkZWZnaGlqa2xtbm9wcXJzdHV2d3h5ent8fX5_gIGCg4SFhoeIiYqLjI2Oj5CRkpOUlZaXmJmam5ydnp-goaKjpKWmp6ipqqusra6vsLGys7S1tre4ubq7vL2-v8DBwsPExcbHyMnKy8zNzs_Q0dLT1NXW19jZ2tvc3d7f4OHi4-Tl5ufo6err7O3u7_Dx8vP09fb3-Pn6-_z9_v8AAAAAAAvbKAEBuzE0OS4xNTQuMTY3LjkxAAA";
const LEGACY_NO_API_ID: &str = "BAAAAQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyAhIiMkJSYnKCkqKywtLi8wMTIzNDU2Nzg5Ojs8PT4_QEFCQ0RFRkdISUpLTE1OT1BRUlNUVVZXWFlaW1xdXl9gYWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXp7fH1-f4CBgoOEhYaHiImKi4yNjo-QkZKTlJWWl5iZmpucnZ6foKGio6SlpqeoqaqrrK2ur7CxsrO0tba3uLm6u7y9vr_AwcLDxMXGx8jJysvMzc7P0NHS09TV1tfY2drb3N3e3-Dh4uPk5ebn6Onq6-zt7u_w8fLz9PX29_j5-vv8_f7_AAAAASoF8gAB";
const LEGACY_U32_USER: &str = "BAAAAQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyAhIiMkJSYnKCkqKywtLi8wMTIzNDU2Nzg5Ojs8PT4_QEFCQ0RFRkdISUpLTE1OT1BRUlNUVVZXWFlaW1xdXl9gYWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXp7fH1-f4CBgoOEhYaHiImKi4yNjo-QkZKTlJWWl5iZmpucnZ6foKGio6SlpqeoqaqrrK2ur7CxsrO0tba3uLm6u7y9vr_AwcLDxMXGx8jJysvMzc7P0NHS09TV1tfY2drb3N3e3-Dh4uPk5ebn6Onq6-zt7u_w8fLz9PX29_j5-vv8_f7_AAvbKAE";

fn check(s: &SessionData) {
    assert_eq!((s.dc_id, s.api_id, s.user_id, s.is_bot), (4, 12345, 777000, true));
    assert!(s.auth_key.iter().enumerate().all(|(i, &b)| b == i as u8));
}

#[test]
fn reads_v3_and_legacy_strings() {
    for s in [V3_STRING, LEGACY_STRING] {
        let s = session_string::parse(s).unwrap();
        check(&s);
        assert_eq!((s.test_mode, s.addr), (false, None));
    }
}

#[test]
fn v4_keeps_test_mode_and_address() {
    let s = session_string::parse(V4_STRING).unwrap();
    check(&s);
    assert_eq!((s.test_mode, s.addr), (true, Some("149.154.167.40:443".parse().unwrap())));
    assert_eq!(session_string::export(&s), V4_STRING);
}

#[test]
fn exports_v4_with_our_prefix() {
    let mut s = session_string::parse(V3_STRING).unwrap();
    for addr in [None, Some("[2001:67c:4e8:f002::a]:443".parse().unwrap())] {
        s.addr = addr;
        let exported = session_string::export(&s);
        assert!(exported.starts_with("VAE_BA"));
        let back = session_string::parse(&exported).unwrap();
        check(&back);
        assert_eq!((back.test_mode, back.addr), (false, addr));
    }
}

#[test]
fn rejects_damaged_strings() {
    for good in [V3_STRING, V4_STRING] {
        let mut damaged = good.to_owned();
        damaged.replace_range(40..41, if &good[40..41] == "A" { "B" } else { "A" });
        assert!(session_string::parse(&damaged).err().unwrap().contains("CRC"));
    }
    assert!(session_string::parse("VAE_AAAA").is_err());
    assert!(session_string::parse("VAE_not*base64").is_err());
}

#[test]
fn reads_wzgram_strings() {
    let v3 = session_string::parse(WZ_V3).unwrap();
    check(&v3);
    assert_eq!((v3.test_mode, v3.addr), (false, Some("[2001:67c:4e8:f004::b]:443".parse().unwrap())));

    let empty = session_string::parse(WZ_V3_NO_ADDR).unwrap();
    check(&empty);
    assert_eq!((empty.test_mode, empty.addr), (true, None));

    for s in [WZ_V2, WZ_V2_NO_CRC] {
        let s = session_string::parse(s).unwrap();
        check(&s);
        assert_eq!((s.test_mode, s.addr), (false, Some("149.154.167.91:443".parse().unwrap())));
    }

    let back = session_string::parse(&session_string::export(&v3)).unwrap();
    check(&back);
    assert_eq!((back.test_mode, back.addr), (v3.test_mode, v3.addr));

    let mut damaged = WZ_V3.to_string();
    damaged.replace_range(40..41, if &WZ_V3[40..41] == "A" { "B" } else { "A" });
    assert!(session_string::parse(&damaged).err().unwrap().contains("CRC"));
}

#[test]
fn reads_pyrogram_strings_without_api_id() {
    let padded = format!("{LEGACY_U32_USER}==");
    for (s, user_id) in [(LEGACY_NO_API_ID, 5_000_000_000), (LEGACY_U32_USER, 777000), (padded.as_str(), 777000)] {
        let s = session_string::parse(s).unwrap();
        assert_eq!((s.dc_id, s.api_id, s.user_id, s.is_bot, s.test_mode, s.addr), (4, 0, user_id, true, false, None));
        assert!(s.auth_key.iter().enumerate().all(|(i, &b)| b == i as u8));
    }
}

fn peers() -> Peers {
    let user = types::InputPeerUser { user_id: 777000, access_hash: -5 };
    let channel = types::InputPeerChannel { channel_id: 1234, access_hash: i64::MAX };
    Peers::from([
        (777000, base::InputPeer::InputPeerUser(Box::new(user))),
        (-1_000_000_001_234, base::InputPeer::InputPeerChannel(Box::new(channel))),
    ])
}

fn update_state() -> UpdateState {
    UpdateState { pts: 10, qts: 2, date: 1_700_000_000, seq: 5, channels: HashMap::from([(1234, 99)]) }
}

const SALT: Salt = Salt { value: -7, time_offset: 3 };

#[test]
fn state_file_round_trips() {
    let mut with_group = peers();
    with_group.insert(-42, base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 42 }));
    let bytes = state::encode(1, SALT, &with_group, Some(&update_state()));
    assert_eq!(state::decode(&bytes, 1), Some((peers(), Some(update_state()), SALT)));
    let bytes = state::encode(1, SALT, &peers(), None);
    assert_eq!(state::decode(&bytes, 1), Some((peers(), None, SALT)));
}

#[test]
fn reads_version_1_state_without_salt() {
    let mut v1 = state::encode(1, Salt::default(), &peers(), Some(&update_state()));
    v1.drain(9..25);
    v1[0] = 1;
    assert_eq!(state::decode(&v1, 1), Some((peers(), Some(update_state()), Salt::default())));
}

#[test]
fn state_file_is_per_account_and_checked() {
    let bytes = state::encode(1, SALT, &peers(), Some(&update_state()));
    assert_eq!(state::decode(&bytes, 2), Some((Peers::new(), None, Salt::default())));
    assert_eq!(state::decode(&bytes[..bytes.len() - 1], 1), None);
    assert_eq!(state::decode(&[2], 1), None);
    assert_eq!(state::decode(&[3], 1), None);
}

#[test]
fn state_file_saves_and_ignores_damage() {
    let path = std::env::temp_dir().join(format!("vaegram-{}.state", std::process::id()));
    assert_eq!(state::load(&path, 1).unwrap(), Default::default());
    write_private(&path, &state::encode(1, SALT, &peers(), Some(&update_state()))).unwrap();
    let loaded = state::load(&path, 1);
    std::fs::write(&path, [1, 2, 3]).unwrap();
    let damaged = state::load(&path, 1);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(loaded.unwrap(), (peers(), Some(update_state()), SALT));
    assert_eq!(damaged.unwrap(), Default::default());
}
