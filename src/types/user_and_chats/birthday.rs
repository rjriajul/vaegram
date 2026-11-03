use crate::raw::base;

/// A user's birthday, with the year only when they show it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Birthday {
    pub day: i32,
    pub month: i32,
    /// Hidden by the owner or never set.
    pub year: Option<i32>,
}

impl Birthday {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn parse(raw: &base::Birthday) -> Birthday {
        let base::Birthday::Birthday(b) = raw;
        Birthday { day: b.day, month: b.month, year: b.year }
    }
}

#[cfg(test)]
mod tests {
    use super::Birthday;
    use crate::raw::{base, types};

    #[test]
    fn parses_with_and_without_a_year() {
        let parse =
            |year| Birthday::parse(&base::Birthday::Birthday(types::Birthday { day: 31, month: 10, year }.into()));
        assert_eq!(parse(Some(1990)), Birthday { day: 31, month: 10, year: Some(1990) });
        assert_eq!(parse(None), Birthday { day: 31, month: 10, year: None });
    }
}
