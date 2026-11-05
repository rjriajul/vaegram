/// The kind of a [`MessageEntity`](crate::types::MessageEntity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MessageEntityType {
    /// An `@username` mention.
    Mention,
    /// A `#hashtag`.
    Hashtag,
    /// A `$CASHTAG`.
    Cashtag,
    /// A bot command such as `/start`.
    BotCommand,
    /// A URL written out in the text.
    Url,
    /// An email address.
    Email,
    /// A phone number.
    PhoneNumber,
    /// Bold text.
    Bold,
    /// Italic text.
    Italic,
    /// Underlined text.
    Underline,
    /// Struck-through text.
    Strikethrough,
    /// Text hidden until tapped.
    Spoiler,
    /// Inline monospace text.
    Code,
    /// A monospace block. The entity's `language` field holds the language, if any.
    Pre,
    /// A quoted block.
    Blockquote,
    /// A quoted block that is collapsed until expanded.
    ExpandableBlockquote,
    /// Text that links to the entity's `url` field.
    TextLink,
    /// Text that mentions the user in the entity's `user` field.
    TextMention,
    /// A custom emoji. The entity's `custom_emoji_id` field identifies it.
    CustomEmoji,
    /// A bank card number.
    BankCard,
    /// A date and time. The entity's `unix_time` and `date_time_format` fields describe it.
    DateTime,
    /// A TON blockchain address.
    TonAddress,
    /// Inserted text in a diff.
    DiffInsert,
    /// Replaced text in a diff. The entity's `old_text` field holds the old text.
    DiffReplace,
    /// Deleted text in a diff.
    DiffDelete,
    /// An entity type Telegram sent as unknown.
    Unknown,
}
