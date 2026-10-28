//! TL objects as JSON, keyed by their original TL names: `{"_":"messageMediaPhoto","spoiler":false,...}`.
//!
//! Flag fields that are absent are left out, `true` flags are booleans, `bytes`/`int128`/`int256` are
//! base64url without padding, and a `long` is a number only while JavaScript can hold it exactly
//! (|v| ≤ 2^53); past that it is a string, so ids and access hashes survive `JSON.parse`.

use std::fmt::Write;

pub trait ToJson {
    fn to_json(&self, out: &mut String);

    fn json(&self) -> String {
        let mut out = String::new();
        self.to_json(&mut out);
        out
    }
}

impl ToJson for i32 {
    fn to_json(&self, out: &mut String) {
        write!(out, "{self}").unwrap();
    }
}

impl ToJson for i64 {
    fn to_json(&self, out: &mut String) {
        if self.unsigned_abs() <= 1 << 53 {
            write!(out, "{self}").unwrap();
        } else {
            write!(out, "\"{self}\"").unwrap();
        }
    }
}

impl ToJson for f64 {
    fn to_json(&self, out: &mut String) {
        if self.is_finite() { write!(out, "{self}").unwrap() } else { out.push_str("null") }
    }
}

impl ToJson for bool {
    fn to_json(&self, out: &mut String) {
        out.push_str(if *self { "true" } else { "false" });
    }
}

impl ToJson for str {
    fn to_json(&self, out: &mut String) {
        out.push('"');
        for c in self.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if c < ' ' => write!(out, "\\u{:04x}", c as u32).unwrap(),
                c => out.push(c),
            }
        }
        out.push('"');
    }
}

impl ToJson for String {
    fn to_json(&self, out: &mut String) {
        self.as_str().to_json(out);
    }
}

impl ToJson for [u8] {
    fn to_json(&self, out: &mut String) {
        out.push('"');
        out.push_str(&crate::utils::base64url_encode(self));
        out.push('"');
    }
}

impl ToJson for Vec<u8> {
    fn to_json(&self, out: &mut String) {
        self.as_slice().to_json(out);
    }
}

impl<const N: usize> ToJson for [u8; N] {
    fn to_json(&self, out: &mut String) {
        self.as_slice().to_json(out);
    }
}

impl<T: ToJson> ToJson for Vec<T> {
    fn to_json(&self, out: &mut String) {
        out.push('[');
        for (i, x) in self.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            x.to_json(out);
        }
        out.push(']');
    }
}

impl<T: ToJson + ?Sized> ToJson for Box<T> {
    fn to_json(&self, out: &mut String) {
        (**self).to_json(out);
    }
}

#[cfg(test)]
mod tests {
    use super::ToJson;
    use crate::raw::{base, functions, types};

    #[test]
    fn primitives() {
        assert_eq!((1i64 << 53).json(), "9007199254740992");
        assert_eq!((-(1i64 << 53) - 1).json(), "\"-9007199254740993\"");
        assert_eq!("a\"\\\n\u{1}é".json(), r#""a\"\\\n\u0001é""#);
        assert_eq!(vec![0xffu8, 0xfe].json(), r#""__4""#);
        assert_eq!(f64::NAN.json(), "null");
        assert_eq!(vec![vec![1i32, 2], vec![]].json(), "[[1,2],[]]");
    }

    #[test]
    fn constructors() {
        assert_eq!(base::InputPeer::InputPeerSelf.json(), r#"{"_":"inputPeerSelf"}"#);
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser {
            user_id: 42,
            access_hash: -7_000_000_000_000_000_000,
        }));
        assert_eq!(peer.json(), r#"{"_":"inputPeerUser","user_id":42,"access_hash":"-7000000000000000000"}"#);
    }

    #[test]
    fn flags() {
        let media = base::MessageMedia::MessageMediaPhoto(Box::new(types::MessageMediaPhoto {
            spoiler: true,
            live_photo: false,
            photo: Some(base::Photo::PhotoEmpty(Box::new(types::PhotoEmpty { id: 5 }))),
            ttl_seconds: None,
            video: None,
        }));
        assert_eq!(
            media.json(),
            r#"{"_":"messageMediaPhoto","spoiler":true,"live_photo":false,"photo":{"_":"photoEmpty","id":5}}"#
        );
    }

    #[test]
    fn generic_function() {
        let call = functions::InvokeWithLayer { layer: 232, query: functions::help::GetConfig {} };
        assert_eq!(call.json(), r#"{"_":"invokeWithLayer","layer":232,"query":{"_":"help.getConfig"}}"#);
    }
}
