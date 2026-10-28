/// The checklist is the message this service message replies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecklistTasksDone {
    pub marked_as_done_task_ids: Vec<i32>,
    pub marked_as_not_done_task_ids: Vec<i32>,
}
