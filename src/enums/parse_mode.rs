#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ParseMode {
    /// The text is sent as it is.
    #[default]
    Disabled,
    Html,
    Markdown,
    /// Markdown and HTML together: `**bold** and <i>italic</i>` in one text. Neither is read inside code, and a
    /// literal `<`, `&` or Markdown character is escaped with a backslash.
    Default,
}
