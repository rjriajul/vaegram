/// What the account is in a community.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CommunityMemberStatus {
    Creator,
    Administrator,
    Member,
    Left,
    /// The community is closed to this account.
    Banned,
}
