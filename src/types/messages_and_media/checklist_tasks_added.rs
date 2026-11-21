use crate::types::ChecklistTask;

/// Tasks added to a checklist.
///
/// The checklist is the message this service message replies to.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ChecklistTasksAdded {
    /// The tasks that were added.
    pub tasks: Vec<ChecklistTask>,
}
