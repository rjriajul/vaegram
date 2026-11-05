/// The account's standing in a community.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CommunityMemberStatus {
    /// Created the community.
    Creator,
    /// Has administrator rights.
    Administrator,
    /// A regular member.
    Member,
    /// Left the community.
    Left,
    /// The community is closed to this account.
    Banned,
}
