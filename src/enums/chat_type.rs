#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChatType {
    Private,
    Bot,
    Group,
    Supergroup,
    Channel,
}
