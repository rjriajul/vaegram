/// Which keyboard button to press: `0` (counting row by row), `(row, column)` or its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ButtonRef<'a> {
    /// Zero-based position counting buttons row by row.
    Index(usize),
    /// Zero-based row and column.
    Position(usize, usize),
    /// The button whose label equals this text exactly.
    Text(&'a str),
}

impl From<usize> for ButtonRef<'_> {
    fn from(i: usize) -> Self {
        ButtonRef::Index(i)
    }
}

impl From<(usize, usize)> for ButtonRef<'_> {
    fn from((row, column): (usize, usize)) -> Self {
        ButtonRef::Position(row, column)
    }
}

impl<'a> From<&'a str> for ButtonRef<'a> {
    fn from(text: &'a str) -> Self {
        ButtonRef::Text(text)
    }
}
