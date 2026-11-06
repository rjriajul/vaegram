use crate::raw::base;

/// A tab on a profile page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProfileTab {
    /// Posts.
    Posts,
    /// Gifts.
    Gifts,
    /// Photos and videos.
    Media,
    /// Files.
    Files,
    /// Music.
    Music,
    /// Voice messages.
    Voice,
    /// Shared links.
    Links,
    /// GIFs.
    Gifs,
}

impl ProfileTab {
    pub(crate) fn parse(raw: &base::ProfileTab) -> ProfileTab {
        use base::ProfileTab as T;
        match raw {
            T::ProfileTabPosts => Self::Posts,
            T::ProfileTabGifts => Self::Gifts,
            T::ProfileTabMedia => Self::Media,
            T::ProfileTabFiles => Self::Files,
            T::ProfileTabMusic => Self::Music,
            T::ProfileTabVoice => Self::Voice,
            T::ProfileTabLinks => Self::Links,
            T::ProfileTabGifs => Self::Gifs,
        }
    }

    pub(crate) fn write(self) -> base::ProfileTab {
        use base::ProfileTab as T;
        match self {
            Self::Posts => T::ProfileTabPosts,
            Self::Gifts => T::ProfileTabGifts,
            Self::Media => T::ProfileTabMedia,
            Self::Files => T::ProfileTabFiles,
            Self::Music => T::ProfileTabMusic,
            Self::Voice => T::ProfileTabVoice,
            Self::Links => T::ProfileTabLinks,
            Self::Gifs => T::ProfileTabGifs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ProfileTab;

    #[test]
    fn profile_tabs_write_every_constructor() {
        use ProfileTab as P;
        let tabs = [P::Posts, P::Gifts, P::Media, P::Files, P::Music, P::Voice, P::Links, P::Gifs];
        let written: Vec<_> = tabs.iter().map(|t| t.write()).collect();
        for (i, tab) in written.iter().enumerate() {
            assert!(!written[..i].contains(tab), "{:?} repeats {tab:?}", tabs[i]);
        }
    }

    #[test]
    fn parses_every_tab() {
        use ProfileTab as P;
        for tab in [P::Posts, P::Gifts, P::Media, P::Files, P::Music, P::Voice, P::Links, P::Gifs] {
            assert_eq!(ProfileTab::parse(&tab.write()), tab);
        }
    }
}
