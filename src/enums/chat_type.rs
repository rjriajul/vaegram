/// The kind of a chat.
///
/// Telegram adds chat kinds, so a `match` outside vaegram needs a `_` arm:
///
/// ```
/// use vaegram::enums::ChatType;
///
/// fn is_group(kind: ChatType) -> bool {
///     match kind {
///         ChatType::Group | ChatType::Supergroup => true,
///         ChatType::Private | ChatType::Bot | ChatType::Channel => false,
///         _ => false,
///     }
/// }
/// ```
///
/// ```compile_fail,E0004
/// use vaegram::enums::ChatType;
///
/// fn is_group(kind: ChatType) -> bool {
///     match kind {
///         ChatType::Group | ChatType::Supergroup => true,
///         ChatType::Private | ChatType::Bot | ChatType::Channel => false,
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ChatType {
    /// A private chat with a user.
    Private,
    /// A private chat with a bot.
    Bot,
    /// A basic group.
    Group,
    /// A supergroup.
    Supergroup,
    /// A broadcast channel.
    Channel,
}
