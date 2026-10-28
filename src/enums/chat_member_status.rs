#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChatMemberStatus {
    Owner,
    Administrator,
    Member,
    Restricted,
    Left,
    Banned,
}
