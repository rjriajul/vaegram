use crate::raw::base;

/// The built-in theme a chat theme builds on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BaseTheme {
    /// Telegram's classic theme.
    #[default]
    Classic,
    /// The light Day theme.
    Day,
    /// The dark Night theme.
    Night,
    /// The tinted variant of the dark theme.
    Tinted,
    /// The tinted variant of the light theme.
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
