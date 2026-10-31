use super::send_options::caption_setters;
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::{PeerRef, Text};

#[must_use = "a request does nothing until it is awaited"]
pub struct AddChecklistTasks<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    tasks: Vec<(i32, Text)>,
    parse_mode: Option<ParseMode>,
}

impl Client {
    pub fn add_checklist_tasks(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> AddChecklistTasks<'_> {
        AddChecklistTasks { client: self, chat_id: chat_id.into(), message_id, tasks: Vec::new(), parse_mode: None }
    }
}

impl AddChecklistTasks<'_> {
    /// Ids must be new to the checklist and positive.
    pub fn task(mut self, id: i32, text: impl Into<Text>) -> Self {
        self.tasks.push((id, text.into()));
        self
    }

    async fn send(self) -> Result<()> {
        if self.tasks.is_empty() {
            return Err(Error::InvalidArgument("add at least one task"));
        }
        let client = self.client;
        let peer = client.resolve_peer(self.chat_id).await?;
        let mut list = Vec::with_capacity(self.tasks.len());
        for (id, text) in self.tasks {
            let (text, entities) = client.write_text(text, self.parse_mode).await?;
            let title = base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities { text, entities }));
            list.push(base::TodoItem::TodoItem(Box::new(types::TodoItem { id, title })));
        }
        client.invoke(&functions::messages::AppendTodoList { peer, msg_id: self.message_id, list }).await?;
        Ok(())
    }
}

caption_setters!(AddChecklistTasks);
into_future!(AddChecklistTasks, ());

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc::UnboundedReceiver;

    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::Text;

    fn user() -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 2 }))
    }

    fn updates() -> Vec<u8> {
        let updates = types::Updates { updates: Vec::new(), users: Vec::new(), chats: Vec::new(), date: 1, seq: 0 };
        base::Updates::Updates(Box::new(updates)).to_bytes()
    }

    fn setup(replies: usize) -> (Client, UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::scripted((0..replies).map(|_| Ok(updates())).collect());
        client.0.peers.lock().unwrap().insert(7, user());
        (client, rx)
    }

    fn item(id: i32, text: &str, entities: Vec<base::MessageEntity>) -> base::TodoItem {
        let title = types::TextWithEntities { text: text.into(), entities };
        let title = base::TextWithEntities::TextWithEntities(Box::new(title));
        base::TodoItem::TodoItem(Box::new(types::TodoItem { id, title }))
    }

    #[tokio::test]
    async fn appends_tasks_with_their_formatting() {
        let (client, mut rx) = setup(1);
        client.add_checklist_tasks(7, 5).task(2, "**Visa**").markdown().task(3, "Hotel").await.unwrap();
        let bold = base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 4 });
        let list = vec![item(2, "Visa", vec![bold]), item(3, "Hotel", Vec::new())];
        let expected = functions::messages::AppendTodoList { peer: user(), msg_id: 5, list };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_task_mention_is_resolved() {
        let (client, mut rx) = setup(1);
        let ann = types::InputPeerUser { user_id: 8, access_hash: 3 };
        client.0.peers.lock().unwrap().insert(8, base::InputPeer::InputPeerUser(Box::new(ann)));
        client.add_checklist_tasks(7, 5).task(1, Text::new().mention("Ann", 8)).await.unwrap();
        let user_id = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 8, access_hash: 3 }));
        let mention = types::InputMessageEntityMentionName { offset: 0, length: 3, user_id };
        let mention = base::MessageEntity::InputMessageEntityMentionName(Box::new(mention));
        let list = vec![item(1, "Ann", vec![mention])];
        let expected = functions::messages::AppendTodoList { peer: user(), msg_id: 5, list };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn no_tasks_is_refused_without_a_request() {
        let (client, mut rx) = setup(0);
        let result = client.add_checklist_tasks(7, 5).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn telegram_errors_pass_through() {
        let invalid = || Err(Error::Rpc(RpcError::new(400, "MESSAGE_ID_INVALID")));
        let (client, _rx) = Client::scripted(vec![invalid(), invalid()]);
        client.0.peers.lock().unwrap().insert(7, user());
        let added = client.add_checklist_tasks(7, 5).task(2, "Visa").await;
        let marked = client.mark_checklist_tasks_as_done(7, 5, &[1], &[]).await;
        for result in [added, marked] {
            assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "MESSAGE_ID_INVALID"), "{result:?}");
        }
    }
}
