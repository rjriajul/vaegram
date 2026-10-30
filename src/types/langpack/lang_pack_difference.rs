use crate::raw::base;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LangPackDifference {
    pub lang_code: String,
    pub from_version: i32,
    pub version: i32,
    pub strings: Vec<LangPackString>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LangPackString {
    Plain {
        key: String,
        value: String,
    },
    Pluralized {
        key: String,
        zero_value: Option<String>,
        one_value: Option<String>,
        two_value: Option<String>,
        few_value: Option<String>,
        many_value: Option<String>,
        other_value: String,
    },
    Deleted {
        key: String,
    },
}

impl LangPackDifference {
    pub(crate) fn parse(raw: base::LangPackDifference) -> LangPackDifference {
        let base::LangPackDifference::LangPackDifference(d) = raw;
        let strings = d.strings.into_iter().map(LangPackString::parse).collect();
        LangPackDifference { lang_code: d.lang_code, from_version: d.from_version, version: d.version, strings }
    }
}

impl LangPackString {
    pub fn key(&self) -> &str {
        match self {
            LangPackString::Plain { key, .. }
            | LangPackString::Pluralized { key, .. }
            | LangPackString::Deleted { key } => key,
        }
    }

    pub(crate) fn parse(raw: base::LangPackString) -> LangPackString {
        use base::LangPackString as S;
        match raw {
            S::LangPackString(s) => LangPackString::Plain { key: s.key, value: s.value },
            S::LangPackStringPluralized(s) => LangPackString::Pluralized {
                key: s.key,
                zero_value: s.zero_value,
                one_value: s.one_value,
                two_value: s.two_value,
                few_value: s.few_value,
                many_value: s.many_value,
                other_value: s.other_value,
            },
            S::LangPackStringDeleted(s) => LangPackString::Deleted { key: s.key },
        }
    }
}
