use crate::raw::base;

/// State of the account's stories stealth mode, which hides views of other people's stories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct StoriesStealthMode {
    /// Unix time when stealth mode ends. `None` if it isn't active.
    pub active_until_date: Option<i32>,
    /// Unix time when stealth mode can be activated again. `None` if there's no cooldown.
    pub cooldown_until_date: Option<i32>,
}

impl StoriesStealthMode {
    pub(crate) fn parse(mode: base::StoriesStealthMode) -> StoriesStealthMode {
        let base::StoriesStealthMode::StoriesStealthMode(m) = mode;
        StoriesStealthMode { active_until_date: m.active_until_date, cooldown_until_date: m.cooldown_until_date }
    }
}
