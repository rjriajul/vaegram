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
}
