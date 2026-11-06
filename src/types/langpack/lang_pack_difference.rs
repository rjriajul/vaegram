use crate::raw::base;

/// The strings of a language pack that changed between two versions.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct LangPackDifference {
    /// Language code of the pack.
    pub lang_code: String,
    /// Version the difference starts from.
    pub from_version: i32,
    /// Version of the pack after applying the difference.
    pub version: i32,
    /// The changed strings.
    pub strings: Vec<LangPackString>,
}

/// A string of a language pack.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LangPackString {
    /// A string with a single form.
    #[non_exhaustive]
    Plain {
        /// Key of the string.
        key: String,
        /// Translated text.
        value: String,
    },
    /// A string with one form per plural category of the language.
    #[non_exhaustive]
    Pluralized {
        /// Key of the string.
        key: String,
        /// Form for zero. `None` if the language has no separate form.
        zero_value: Option<String>,
        /// Form for one. `None` if the language has no separate form.
        one_value: Option<String>,
        /// Form for two. `None` if the language has no separate form.
        two_value: Option<String>,
        /// Form for a few. `None` if the language has no separate form.
        few_value: Option<String>,
        /// Form for many. `None` if the language has no separate form.
        many_value: Option<String>,
        /// Form used when no other form applies.
        other_value: String,
    },
    /// A string that was removed from the pack.
    #[non_exhaustive]
    Deleted {
        /// Key of the removed string.
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
    /// Returns the key of the string, whichever variant it is.
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
