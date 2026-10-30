use crate::raw::base;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Country {
    pub iso2: String,
    pub default_name: String,
    pub name: Option<String>,
    pub hidden: bool,
    pub country_codes: Vec<CountryCode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountryCode {
    pub country_code: String,
    pub prefixes: Vec<String>,
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
        Country { iso2: c.iso2, default_name: c.default_name, name: c.name, hidden: c.hidden, country_codes }
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
        assert_eq!((country.hidden, country.name, country.country_codes), (true, None, vec![code]));
    }
}
