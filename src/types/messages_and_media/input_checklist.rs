use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::{Checklist, Text};

/// A checklist to send, built from a title and its tasks.
///
/// Convert a received [`Checklist`] with `From<&Checklist>` to resend it with its formatting kept.
#[derive(Debug, Clone)]
pub struct InputChecklist {
    // Entities are `Some` only for texts taken from a received checklist; those skip the parse mode.
    title: (Text, Option<Vec<base::MessageEntity>>),
    tasks: Vec<(i32, Text, Option<Vec<base::MessageEntity>>)>,
    parse_mode: Option<ParseMode>,
    others_can_add_tasks: bool,
    others_can_mark_tasks_as_done: bool,
}

impl InputChecklist {
    /// Starts a checklist with the given title and no tasks.
    pub fn new(title: impl Into<Text>) -> Self {
        InputChecklist {
            title: (title.into(), None),
            tasks: Vec::new(),
            parse_mode: None,
            others_can_add_tasks: false,
            others_can_mark_tasks_as_done: false,
        }
    }

    /// Adds a task to the checklist.
    ///
    /// Task ids must be positive and unique within the checklist.
    #[must_use]
    pub fn task(mut self, id: i32, text: impl Into<Text>) -> Self {
        self.tasks.push((id, text.into(), None));
        self
    }

    /// Parses the title and the tasks added with [`task`](InputChecklist::task) as HTML.
    #[must_use]
    pub fn html(self) -> Self {
        self.parse_mode(ParseMode::Html)
    }

    /// Parses the title and the tasks added with [`task`](InputChecklist::task) as Markdown.
    #[must_use]
    pub fn markdown(self) -> Self {
        self.parse_mode(ParseMode::Markdown)
    }

    /// Parses the title and the tasks added with [`task`](InputChecklist::task) with the given mode.
    ///
    /// Texts copied from a received checklist keep their entities and skip the mode.
    #[must_use]
    pub fn parse_mode(mut self, mode: ParseMode) -> Self {
        self.parse_mode = Some(mode);
        self
    }

    /// Lets other users add tasks to the checklist.
    #[must_use]
    pub fn others_can_add_tasks(mut self) -> Self {
        self.others_can_add_tasks = true;
        self
    }

    /// Lets other users mark tasks as done.
    #[must_use]
    pub fn others_can_mark_tasks_as_done(mut self) -> Self {
        self.others_can_mark_tasks_as_done = true;
        self
    }

    pub(crate) async fn write(self, client: &Client) -> Result<base::InputMedia> {
        let mode = self.parse_mode;
        let title = text_with_entities(client, self.title, mode).await?;
        let mut list = Vec::with_capacity(self.tasks.len());
        for (id, text, entities) in self.tasks {
            let title = text_with_entities(client, (text, entities), mode).await?;
            list.push(base::TodoItem::TodoItem(Box::new(types::TodoItem { id, title })));
        }
        let todo = types::TodoList {
            others_can_append: self.others_can_add_tasks,
            others_can_complete: self.others_can_mark_tasks_as_done,
            title,
            list,
        };
        let todo = base::TodoList::TodoList(Box::new(todo));
        Ok(base::InputMedia::InputMediaTodo(Box::new(types::InputMediaTodo { todo })))
    }
}

impl From<&Checklist> for InputChecklist {
    fn from(checklist: &Checklist) -> Self {
        let base::TodoList::TodoList(todo) = &checklist.todo;
        let text = |t: &base::TextWithEntities| {
            let base::TextWithEntities::TextWithEntities(t) = t;
            (Text::from(t.text.clone()), Some(t.entities.clone()))
        };
        let tasks = todo
            .list
            .iter()
            .map(|base::TodoItem::TodoItem(item)| {
                let (text, entities) = text(&item.title);
                (item.id, text, entities)
            })
            .collect();
        InputChecklist {
            title: text(&todo.title),
            tasks,
            parse_mode: None,
            others_can_add_tasks: todo.others_can_append,
            others_can_mark_tasks_as_done: todo.others_can_complete,
        }
    }
}

// Telegram only accepts user mentions in their input form, so received ones are resolved again.
async fn text_with_entities(
    client: &Client,
    (text, entities): (Text, Option<Vec<base::MessageEntity>>),
    mode: Option<ParseMode>,
) -> Result<base::TextWithEntities> {
    let (text, entities) = match entities {
        Some(entities) => (text.text, client.resolve_mentions(entities).await?),
        None => client.write_text(text, mode).await?,
    };
    Ok(base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities { text, entities })))
}

#[cfg(test)]
mod tests {
    use super::super::checklist::tests::text;
    use super::InputChecklist;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::enums::ParseMode;
    use crate::raw::{base, types};
    use crate::types::{Checklist, Text};

    #[tokio::test]
    async fn keeps_received_formatting_and_parses_new_tasks() {
        let client = Client::offline();
        let peer = types::InputPeerUser { user_id: 42, access_hash: 7 };
        client.0.peers.lock().unwrap().insert(42, base::InputPeer::InputPeerUser(Box::new(peer)));
        let mention = types::MessageEntityMentionName { offset: 0, length: 3, user_id: 42 };
        let mention = base::MessageEntity::MessageEntityMentionName(Box::new(mention));
        let received = types::MessageMediaToDo {
            todo: base::TodoList::TodoList(Box::new(types::TodoList {
                others_can_append: true,
                others_can_complete: false,
                title: text("1 <b> 2", Vec::new()),
                list: vec![base::TodoItem::TodoItem(Box::new(types::TodoItem {
                    id: 1,
                    title: text("Ann books", vec![mention]),
                }))],
            })),
            completions: None,
        };
        let received = Checklist::parse(&received, &Entities::default());

        let input = InputChecklist::from(&received).task(2, "<b>Hotel</b>").parse_mode(ParseMode::Html);
        let base::InputMedia::InputMediaTodo(media) = input.write(&client).await.unwrap() else {
            panic!("not a checklist")
        };
        let base::TodoList::TodoList(todo) = &media.todo;
        assert_eq!((todo.others_can_append, todo.others_can_complete), (true, false));

        let user_id = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 42, access_hash: 7 }));
        let mention = types::InputMessageEntityMentionName { offset: 0, length: 3, user_id };
        let bold = types::MessageEntityBold { offset: 0, length: 5 };
        let tasks: Vec<_> =
            todo.list.iter().map(|base::TodoItem::TodoItem(item)| (item.id, item.title.clone())).collect();
        // A received text without entities stays literal even though new tasks are parsed as HTML.
        assert_eq!(todo.title, text("1 <b> 2", Vec::new()));
        assert_eq!(
            tasks,
            vec![
                (1, text("Ann books", vec![base::MessageEntity::InputMessageEntityMentionName(Box::new(mention))])),
                (2, text("Hotel", vec![base::MessageEntity::MessageEntityBold(bold)])),
            ]
        );
    }

    #[tokio::test]
    async fn built_texts_keep_their_entities() {
        let client = Client::offline();
        let input = InputChecklist::new(Text::new().bold("Trip")).task(1, Text::new().bold("Hotel")).html();
        let base::InputMedia::InputMediaTodo(media) = input.write(&client).await.unwrap() else {
            panic!("not a checklist")
        };
        let base::TodoList::TodoList(todo) = &media.todo;
        let bold =
            |length| vec![base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length })];
        assert_eq!(todo.title, text("Trip", bold(4)));
        let base::TodoItem::TodoItem(task) = &todo.list[0];
        assert_eq!((task.id, &task.title), (1, &text("Hotel", bold(5))));
    }
}
