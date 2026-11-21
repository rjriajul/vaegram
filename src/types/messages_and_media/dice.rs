/// An animated dice-style emoji and the value it landed on.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Dice {
    /// The emoji that was thrown.
    pub emoji: String,
    /// The result of the throw.
    pub value: i32,
}
