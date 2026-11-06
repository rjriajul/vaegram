/// Review state of a post suggested to a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SuggestedPostState {
    /// Not approved or declined yet.
    Pending,
    /// The channel accepted the post.
    Approved,
    /// The channel rejected the post.
    Declined,
}
