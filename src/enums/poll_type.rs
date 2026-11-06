/// Whether a poll is a regular poll or a quiz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PollType {
    /// A poll with no correct answer.
    Regular,
    /// A poll with a correct answer.
    Quiz,
}
