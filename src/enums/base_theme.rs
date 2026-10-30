use crate::raw::base;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BaseTheme {
    #[default]
    Classic,
    Day,
    Night,
    Tinted,
    Arctic,
}

impl BaseTheme {
    pub(crate) fn parse(theme: &base::BaseTheme) -> Self {
        use base::BaseTheme as T;
        match theme {
            T::BaseThemeClassic => Self::Classic,
            T::BaseThemeDay => Self::Day,
            T::BaseThemeNight => Self::Night,
            T::BaseThemeTinted => Self::Tinted,
            T::BaseThemeArctic => Self::Arctic,
        }
    }

    pub(crate) fn write(self) -> base::BaseTheme {
        use base::BaseTheme as T;
        match self {
            Self::Classic => T::BaseThemeClassic,
            Self::Day => T::BaseThemeDay,
            Self::Night => T::BaseThemeNight,
            Self::Tinted => T::BaseThemeTinted,
            Self::Arctic => T::BaseThemeArctic,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BaseTheme;

    #[test]
    fn round_trips_through_raw() {
        for t in [BaseTheme::Classic, BaseTheme::Day, BaseTheme::Night, BaseTheme::Tinted, BaseTheme::Arctic] {
            assert_eq!(BaseTheme::parse(&t.write()), t);
        }
    }
}
