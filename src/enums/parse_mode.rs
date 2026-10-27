#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ParseMode {
    #[default]
    Disabled,
    Html,
    Markdown,
}
