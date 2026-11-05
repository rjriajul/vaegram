/// How outgoing text is turned into formatting entities.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ParseMode {
    /// The text is sent as it is.
    #[default]
    Disabled,
    /// The text is read as HTML tags.
    Html,
    /// The text is read as Markdown.
    Markdown,
    /// Markdown and HTML together: `**bold** and <i>italic</i>` in one text. Neither is read inside code, and a
    /// literal `<`, `&` or Markdown character is escaped with a backslash.
    Default,
}
