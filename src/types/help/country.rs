use crate::raw::base;

/// A country in Telegram's list of phone number countries.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Country {
    /// Two-letter ISO 3166-1 code.
    pub iso2: String,
    /// Country name in English.
    pub default_name: String,
    /// Name in the language Telegram was asked for. `None` if Telegram sent no localized name.
    pub name: Option<String>,
    /// Set when Telegram marks the country as hidden.
    pub is_hidden: bool,
    /// Dialing codes of the country.
    pub country_codes: Vec<CountryCode>,
}

/// A dialing code of a [`Country`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CountryCode {
    /// The code without a leading plus, for example `1`.
    pub country_code: String,
    /// Number prefixes that follow the code. Empty if Telegram sent none.
    pub prefixes: Vec<String>,
    /// Formatting patterns for the number, with `X` standing for a digit. Empty if Telegram sent none.
    pub patterns: Vec<String>,
}

impl Country {
    pub(crate) fn parse(raw: base::help::Country) -> Country {
        let base::help::Country::Country(c) = raw;
        let country_codes = c
            .country_codes
            .into_iter()
            .map(|base::help::CountryCode::CountryCode(code)| CountryCode {
                country_code: code.country_code,
                prefixes: code.prefixes.unwrap_or_default(),
                patterns: code.patterns.unwrap_or_default(),
            })
            .collect();
        Country { iso2: c.iso2, default_name: c.default_name, name: c.name, is_hidden: c.hidden, country_codes }
    }
}

#[cfg(test)]
mod tests {
    use super::{Country, CountryCode};
    use crate::raw::{base, types};

    #[test]
    fn a_code_without_prefixes_or_patterns_parses_empty() {
        let code = types::help::CountryCode {
            country_code: "1".into(),
            prefixes: None,
            patterns: Some(vec!["XXX XXXX".into()]),
        };
        let raw = types::help::Country {
            hidden: true,
            iso2: "US".into(),
            default_name: "United States".into(),
            name: None,
            country_codes: vec![base::help::CountryCode::CountryCode(Box::new(code))],
        };
        let country = Country::parse(base::help::Country::Country(Box::new(raw)));
        let code = CountryCode { country_code: "1".into(), prefixes: Vec::new(), patterns: vec!["XXX XXXX".into()] };
        assert_eq!((country.is_hidden, country.name, country.country_codes), (true, None, vec![code]));
    }
}
