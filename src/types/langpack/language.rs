use crate::raw::base;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language {
    pub name: String,
    pub native_name: String,
    pub lang_code: String,
    pub base_lang_code: Option<String>,
    pub plural_code: String,
    pub strings_count: i32,
    pub translated_count: i32,
    pub translations_url: String,
    pub official: bool,
    pub rtl: bool,
    pub beta: bool,
}

impl Language {
    pub(crate) fn parse(raw: base::LangPackLanguage) -> Language {
        let base::LangPackLanguage::LangPackLanguage(l) = raw;
        Language {
            name: l.name,
            native_name: l.native_name,
            lang_code: l.lang_code,
            base_lang_code: l.base_lang_code,
            plural_code: l.plural_code,
            strings_count: l.strings_count,
            translated_count: l.translated_count,
            translations_url: l.translations_url,
            official: l.official,
            rtl: l.rtl,
            beta: l.beta,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Language;
    use crate::raw::{Deserializable, base};

    #[test]
    fn parses_a_get_languages_reply() {
        let reply = [
            &[0x15, 0xc4, 0xb5, 0x1c, 1, 0, 0, 0][..],
            &[0xe3, 0x5c, 0xca, 0xee, 0b110, 0, 0, 0],
            &[6, b'A', b'r', b'a', b'b', b'i', b'c', 0],
            &[2, b'a', b'r', 0],
            &[6, b'a', b'r', b'-', b'r', b'a', b'w', 0],
            &[2, b'a', b'r', 0],
            &[2, b'a', b'r', 0],
            &[10, 0, 0, 0, 9, 0, 0, 0],
            &[1, b't', 0, 0],
        ]
        .concat();
        let languages: Vec<Language> =
            Vec::<base::LangPackLanguage>::from_bytes(&reply).unwrap().into_iter().map(Language::parse).collect();
        let arabic = Language {
            name: "Arabic".into(),
            native_name: "ar".into(),
            lang_code: "ar-raw".into(),
            base_lang_code: Some("ar".into()),
            plural_code: "ar".into(),
            strings_count: 10,
            translated_count: 9,
            translations_url: "t".into(),
            official: false,
            rtl: true,
            beta: false,
        };
        assert_eq!(languages, [arabic]);
    }
}
