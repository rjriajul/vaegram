/// Which block list a block, unblock or listing call works on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BlockList {
    /// People who can't message the account, and can't see its stories either.
    #[default]
    Main,
    /// People who can't see the account's stories but can still message it.
    Stories,
}

impl BlockList {
    pub(crate) fn my_stories_from(self) -> bool {
        self == BlockList::Stories
    }
}
