use crate::types::ChecklistTask;

/// Tasks added to a checklist.
///
/// The checklist is the message this service message replies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecklistTasksAdded {
    /// The tasks that were added.
    pub tasks: Vec<ChecklistTask>,
}
