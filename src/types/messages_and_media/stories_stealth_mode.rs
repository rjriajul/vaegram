use crate::raw::base;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoriesStealthMode {
    pub active_until_date: Option<i32>,
    pub cooldown_until_date: Option<i32>,
}

impl StoriesStealthMode {
    pub(crate) fn parse(mode: base::StoriesStealthMode) -> StoriesStealthMode {
        let base::StoriesStealthMode::StoriesStealthMode(m) = mode;
        StoriesStealthMode { active_until_date: m.active_until_date, cooldown_until_date: m.cooldown_until_date }
    }
}
