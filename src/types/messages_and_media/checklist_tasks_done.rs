/// The checklist is the message this service message replies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecklistTasksDone {
    /// Ids of the tasks that were marked as done.
    pub marked_as_done_task_ids: Vec<i32>,
    /// Ids of the tasks that were marked as not done.
    pub marked_as_not_done_task_ids: Vec<i32>,
}
