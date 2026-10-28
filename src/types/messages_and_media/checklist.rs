use super::text_with_entities;
use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::{Chat, MessageEntity, User};

#[derive(Debug, Clone, PartialEq)]
pub struct Checklist {
    pub title: String,
    pub title_entities: Option<Vec<MessageEntity>>,
    pub tasks: Vec<ChecklistTask>,
    pub others_can_add_tasks: bool,
    pub others_can_mark_tasks_as_done: bool,
    // Kept so an edit can send the existing texts back with their formatting untouched.
    pub(crate) todo: base::TodoList,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecklistTask {
    pub id: i32,
    pub text: String,
    pub text_entities: Option<Vec<MessageEntity>>,
    pub completed_by_user: Option<User>,
    pub completed_by_chat: Option<Chat>,
    pub completion_date: Option<i32>,
}

impl Checklist {
    pub(crate) fn parse(media: &types::MessageMediaToDo, entities: &Entities) -> Checklist {
        let base::TodoList::TodoList(list) = &media.todo;
        let completions = media.completions.as_deref().unwrap_or_default();
        let tasks = list
            .list
            .iter()
            .map(|item| {
                let mut task = ChecklistTask::parse(item, entities);
                let done =
                    completions.iter().map(|base::TodoCompletion::TodoCompletion(c)| c).find(|c| c.id == task.id);
                if let Some(done) = done {
                    match &done.completed_by {
                        base::Peer::PeerUser(p) => task.completed_by_user = Some(User::parse(p.user_id, entities)),
                        peer => task.completed_by_chat = Some(Chat::parse(peer, entities, false)),
                    }
                    task.completion_date = Some(done.date);
                }
                task
            })
            .collect();
        let (title, title_entities) = text_with_entities(&list.title, entities);
        Checklist {
            title,
            title_entities,
            tasks,
            others_can_add_tasks: list.others_can_append,
            others_can_mark_tasks_as_done: list.others_can_complete,
            todo: media.todo.clone(),
        }
    }
}

impl ChecklistTask {
    pub(crate) fn parse(item: &base::TodoItem, entities: &Entities) -> ChecklistTask {
        let base::TodoItem::TodoItem(item) = item;
        let (text, text_entities) = text_with_entities(&item.title, entities);
        ChecklistTask {
            id: item.id,
            text,
            text_entities,
            completed_by_user: None,
            completed_by_chat: None,
            completion_date: None,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{Checklist, ChecklistTask};
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};
    use crate::types::User;
    use crate::utils::get_peer;

    pub(crate) fn text(text: &str, entities: Vec<base::MessageEntity>) -> base::TextWithEntities {
        base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities { text: text.into(), entities }))
    }

    pub(crate) fn item(id: i32, title: &str) -> base::TodoItem {
        base::TodoItem::TodoItem(Box::new(types::TodoItem { id, title: text(title, Vec::new()) }))
    }

    #[test]
    fn joins_completions_to_tasks() {
        let done = |id, completed_by, date| {
            base::TodoCompletion::TodoCompletion(Box::new(types::TodoCompletion { id, completed_by, date }))
        };
        let media = types::MessageMediaToDo {
            todo: base::TodoList::TodoList(Box::new(types::TodoList {
                others_can_append: true,
                others_can_complete: false,
                title: text("Trip", Vec::new()),
                list: vec![item(1, "Tickets"), item(2, "Hotel"), item(3, "Visa")],
            })),
            completions: Some(vec![done(3, get_peer(-1000000000005), 20), done(1, get_peer(42), 10)]),
        };

        let entities = Entities::default();
        let checklist = Checklist::parse(&media, &entities);
        let flags = (checklist.title.as_str(), checklist.others_can_add_tasks, checklist.others_can_mark_tasks_as_done);
        assert_eq!(flags, ("Trip", true, false));
        let [tickets, hotel, visa] = &checklist.tasks[..] else { panic!("expected 3 tasks") };
        assert_eq!(tickets.completed_by_user, Some(User { id: 42, ..User::default() }));
        assert_eq!((tickets.completed_by_chat.as_ref(), tickets.completion_date), (None, Some(10)));
        assert_eq!(hotel, &ChecklistTask::parse(&item(2, "Hotel"), &entities));
        assert_eq!(visa.completed_by_chat.as_ref().map(|c| c.id), Some(-1000000000005));
        assert_eq!((visa.completed_by_user.as_ref(), visa.completion_date), (None, Some(20)));
    }
}
