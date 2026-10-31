use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn mark_checklist_tasks_as_done(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        marked_as_done_task_ids: &[i32],
        marked_as_not_done_task_ids: &[i32],
    ) -> Result<()> {
        if marked_as_done_task_ids.is_empty() && marked_as_not_done_task_ids.is_empty() {
            return Err(Error::InvalidArgument("mark at least one task as done or not done"));
        }
        let request = functions::messages::ToggleTodoCompleted {
            peer: self.resolve_peer(chat_id).await?,
            msg_id: message_id,
            completed: marked_as_done_task_ids.to_vec(),
            incompleted: marked_as_not_done_task_ids.to_vec(),
        };
        self.invoke(&request).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};

    fn user() -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 2 }))
    }

    fn updates() -> Vec<u8> {
        let updates = types::Updates { updates: Vec::new(), users: Vec::new(), chats: Vec::new(), date: 1, seq: 0 };
        base::Updates::Updates(Box::new(updates)).to_bytes()
    }

    fn toggled(completed: Vec<i32>, incompleted: Vec<i32>) -> Vec<u8> {
        functions::messages::ToggleTodoCompleted { peer: user(), msg_id: 5, completed, incompleted }.to_bytes()
    }

    #[tokio::test]
    async fn marks_done_and_not_done() {
        let (client, mut rx) = Client::scripted(vec![Ok(updates()), Ok(updates())]);
        client.0.peers.lock().unwrap().insert(7, user());
        client.mark_checklist_tasks_as_done(7, 5, &[1, 3], &[2]).await.unwrap();
        assert_eq!(rx.try_recv().unwrap(), toggled(vec![1, 3], vec![2]));
        client.mark_checklist_tasks_as_done(7, 5, &[], &[2]).await.unwrap();
        assert_eq!(rx.try_recv().unwrap(), toggled(Vec::new(), vec![2]));
    }

    #[tokio::test]
    async fn nothing_to_mark_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        client.0.peers.lock().unwrap().insert(7, user());
        let result = client.mark_checklist_tasks_as_done(7, 5, &[], &[]).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(rx.try_recv().is_err());
    }
}
