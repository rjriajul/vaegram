use crate::raw::base;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProfileTab {
    Posts,
    Gifts,
    Media,
    Files,
    Music,
    Voice,
    Links,
    Gifs,
}

impl ProfileTab {
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
}
