use vaegram::storage::{SessionData, session_string};

const V3_STRING: &str = "VAE_AwQAADA5AAABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4fICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj9AQUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVpbXF1eX2BhYmNkZWZnaGlqa2xtbm9wcXJzdHV2d3h5ent8fX5_gIGCg4SFhoeIiYqLjI2Oj5CRkpOUlZaXmJmam5ydnp-goaKjpKWmp6ipqqusra6vsLGys7S1tre4ubq7vL2-v8DBwsPExcbHyMnKy8zNzs_Q0dLT1NXW19jZ2tvc3d7f4OHi4-Tl5ufo6err7O3u7_Dx8vP09fb3-Pn6-_z9_v8AAAAAAAvbKAEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAYNzrA";
const LEGACY_STRING: &str = "BAAAMDkAAAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8gISIjJCUmJygpKissLS4vMDEyMzQ1Njc4OTo7PD0-P0BBQkNERUZHSElKS0xNTk9QUVJTVFVWV1hZWltcXV5fYGFiY2RlZmdoaWprbG1ub3BxcnN0dXZ3eHl6e3x9fn-AgYKDhIWGh4iJiouMjY6PkJGSk5SVlpeYmZqbnJ2en6ChoqOkpaanqKmqq6ytrq-wsbKztLW2t7i5uru8vb6_wMHCw8TFxsfIycrLzM3Oz9DR0tPU1dbX2Nna29zd3t_g4eLj5OXm5-jp6uvs7e7v8PHy8_T19vf4-fr7_P3-_wAAAAAAC9soAQ";

fn check(s: &SessionData) {
    assert_eq!((s.dc_id, s.api_id, s.user_id, s.is_bot), (4, 12345, 777000, true));
    assert!(s.auth_key.iter().enumerate().all(|(i, &b)| b == i as u8));
}

#[test]
fn reads_v3_and_legacy_strings() {
    check(&session_string::parse(V3_STRING).unwrap());
    check(&session_string::parse(LEGACY_STRING).unwrap());
}

#[test]
fn export_is_v3_with_our_prefix() {
    let s = session_string::parse(V3_STRING).unwrap();
    let exported = session_string::export(&s);
    assert_eq!(exported, V3_STRING);
    check(&session_string::parse(&exported).unwrap());
}

#[test]
fn rejects_damaged_strings() {
    let mut damaged = V3_STRING.to_string();
    damaged.replace_range(40..41, if &V3_STRING[40..41] == "A" { "B" } else { "A" });
    assert!(session_string::parse(&damaged).err().unwrap().contains("CRC"));
    assert!(session_string::parse("VAE_AAAA").is_err());
    assert!(session_string::parse("VAE_not*base64").is_err());
}
