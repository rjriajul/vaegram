use crate::raw::base;

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// `None` for fractions and for numbers outside `i64`.
    pub fn as_i64(&self) -> Option<i64> {
        // 2^63 is exactly representable, so it bounds the range from above; `as` would saturate it to `i64::MAX`.
        let limit = -(i64::MIN as f64);
        match self {
            Json::Number(n) if n.fract() == 0.0 && (-limit..limit).contains(n) => Some(*n as i64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Object(fields) => Some(fields),
            _ => None,
        }
    }

    pub(crate) fn parse(raw: base::JSONValue) -> Json {
        use base::JSONObjectValue as F;
        use base::JSONValue as J;
        match raw {
            J::JsonNull => Json::Null,
            J::JsonBool(b) => Json::Bool(b.value),
            J::JsonNumber(n) => Json::Number(n.value),
            J::JsonString(s) => Json::String(s.value),
            J::JsonArray(a) => Json::Array(a.value.into_iter().map(Json::parse).collect()),
            J::JsonObject(o) => {
                Json::Object(o.value.into_iter().map(|F::JsonObjectValue(f)| (f.key, Json::parse(f.value))).collect())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Json;
    use crate::raw::{Deserializable, Serializable, base, types};

    #[test]
    fn parses_app_config_from_the_wire() {
        use base::JSONValue as J;
        let field = |key: &str, value| {
            base::JSONObjectValue::JsonObjectValue(Box::new(types::JsonObjectValue { key: key.into(), value }))
        };
        let sounds = vec![J::JsonNull, J::JsonString(Box::new(types::JsonString { value: "🎉".into() }))];
        let raw = J::JsonObject(Box::new(types::JsonObject {
            value: vec![
                field("stories_all_hidden", J::JsonBool(types::JsonBool { value: true })),
                field("caption_length_limit_premium", J::JsonNumber(types::JsonNumber { value: 4096.0 })),
                field("emojies_sounds", J::JsonArray(Box::new(types::JsonArray { value: sounds }))),
            ],
        }));
        let config = Json::parse(J::deserialize(&mut raw.to_bytes().as_slice()).unwrap());

        assert_eq!(
            (config.get("stories_all_hidden"), config.get("caption_length_limit_premium"), config.get("missing")),
            (Some(&Json::Bool(true)), Some(&Json::Number(4096.0)), None)
        );
        assert_eq!(config.get("emojies_sounds"), Some(&Json::Array(vec![Json::Null, Json::String("🎉".into())])));
        assert_eq!(Json::Null.get("stories_all_hidden"), None);
    }

    #[test]
    fn typed_accessors() {
        let object = Json::Object(vec![("on".into(), Json::Bool(true))]);
        assert_eq!((object.as_object(), object.as_bool()), (Some(&[("on".into(), Json::Bool(true))][..]), None));
        assert_eq!(object.get("on").and_then(Json::as_bool), Some(true));
        let array = Json::Array(vec![Json::Null]);
        assert_eq!((array.as_array(), array.as_object()), (Some(&[Json::Null][..]), None));
        let text = Json::String("hi".into());
        assert_eq!((text.as_str(), text.as_f64(), Json::Null.as_str()), (Some("hi"), None, None));

        let number = Json::Number;
        assert_eq!((number(4096.0).as_i64(), number(4096.0).as_f64()), (Some(4096), Some(4096.0)));
        assert_eq!((number(-0.0).as_i64(), number(-9.007_199_254_740_992e15).as_i64()), (Some(0), Some(-(1 << 53))));
        assert_eq!((number(i64::MIN as f64).as_i64(), number(-(i64::MIN as f64)).as_i64()), (Some(i64::MIN), None));
        for not_an_i64 in [0.5, -1.5, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e300] {
            assert_eq!(number(not_an_i64).as_i64(), None, "{not_an_i64}");
        }
        assert_eq!((text.as_i64(), Json::Bool(true).as_f64()), (None, None));
    }
}
