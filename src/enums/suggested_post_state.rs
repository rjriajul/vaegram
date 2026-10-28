#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuggestedPostState {
    Pending,
    Approved,
    Declined,
}
